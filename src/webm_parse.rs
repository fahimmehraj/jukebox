use std::{
    io::SeekFrom,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncSeek, BufReader, ReadBuf};
use tokio_stream::Stream;
use tracing::{error, trace, warn};

macro_rules! ready_next {
  ($e:expr) => {
      match $e {
          Poll::Pending => return Poll::Pending,
          Poll::Ready(Some(t)) => t,
          Poll::Ready(None) => return Poll::Ready(None)
          // Poll::Ready(Err(_)) => return Poll::Ready(None),
      }
  };
}

#[derive(Debug, Clone)]
enum ParserStateMachine {
    ReadElementIdLength,
    // u32 is the size of id, u32 is the id so far
    ReadElementId(usize, u32),
    ReadElementSizeLength(),
    ReadElementSize(usize, u64),
    // u64 is the size of the element data
    ReadElementData(usize),
}

#[derive(Debug, Clone, Copy)]
enum EbmlElementId {
    Header,
    DocType,
    Segment,
    SeekHead,
    Info,
    TimecodeScale,
    Tracks,
    TrackEntry,
    CodecID,
    Cluster,
    Timecode,
    SimpleBlock,
    BlockGroup,
    Block,
    Cues,
    Audio,
    AudioChannels,
    Void,
    Unknown(u32), // Use for IDs that don't have a specific variant
}

impl From<u32> for EbmlElementId {
    fn from(id: u32) -> Self {
        match id {
            0x1A45DFA3 => EbmlElementId::Header,
            0x4282 => EbmlElementId::DocType,
            0x18538067 => EbmlElementId::Segment,
            0x114D9B74 => EbmlElementId::SeekHead,
            0x1549A966 => EbmlElementId::Info,
            0x2AD7B1 => EbmlElementId::TimecodeScale,
            0x1654AE6B => EbmlElementId::Tracks,
            0xAE => EbmlElementId::TrackEntry,
            0x86 => EbmlElementId::CodecID,
            0x1F43B675 => EbmlElementId::Cluster,
            0xE7 => EbmlElementId::Timecode,
            0xA3 => EbmlElementId::SimpleBlock,
            0xA0 => EbmlElementId::BlockGroup,
            0xA1 => EbmlElementId::Block,
            0x1C53BB6B => EbmlElementId::Cues,
            0xE1 => EbmlElementId::Audio,
            0x9F => EbmlElementId::AudioChannels,
            0xEC => EbmlElementId::Void,
            _ => EbmlElementId::Unknown(id),
        }
    }
}

pub struct WebmStream<T: AsyncBufRead + AsyncSeek + Unpin> {
    stream: T,
    stack: Vec<EbmlElementId>,
    parser_state: ParserStateMachine,
    buf: Option<Vec<u8>>,
    cursor: usize,
    seek_in_progress: bool,
    simple_blocks: u64,
}

impl<T: AsyncBufRead + AsyncSeek + Unpin> WebmStream<T> {
    fn read_exact_bytes(
        &mut self,
        cx: &mut Context<'_>,
        num_bytes: usize,
    ) -> Poll<Option<Vec<u8>>> {
        let mut buf = self.buf.take().unwrap_or(vec![0u8; num_bytes]);

        let mut read_buf = ReadBuf::new(&mut buf);
        read_buf.advance(self.cursor);

        loop {
            let rem = read_buf.remaining();
            trace!(rem);
            if rem == 0 {
                self.cursor = 0;
                return Poll::Ready(Some(buf));
            }
            match Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
                Poll::Ready(Ok(())) => {
                    if read_buf.remaining() == rem {
                        return Poll::Ready(None);
                    }
                    self.cursor = read_buf.filled().len();
                }
                Poll::Ready(Err(e)) => {
                    error!("error reading stream: {}", e);
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    self.cursor = read_buf.filled().len();
                    self.buf = Some(buf);
                    warn!("read more than once");
                    return Poll::Pending;
                }
            }
        }
    }
}

impl<T: AsyncBufRead + AsyncSeek + Unpin + Send> Stream for WebmStream<T> {
    type Item = Vec<u8>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Vec<u8>>> {
        match self.parser_state {
            ParserStateMachine::ReadElementIdLength => {
                let buf = ready_next!(self.read_exact_bytes(cx, 1));

                let first_byte = buf[0];

                let size = first_byte.leading_zeros() as usize + 1;
                if size == 1 {
                    self.stack.push((first_byte as u32).into());
                    self.parser_state = ParserStateMachine::ReadElementSizeLength();
                    return self.poll_next(cx);
                } else {
                    self.parser_state = ParserStateMachine::ReadElementId(size, first_byte as u32);
                    return self.poll_next(cx);
                }
            }
            ParserStateMachine::ReadElementId(size, mut id) => {
                let id_bytes = ready_next!(self.read_exact_bytes(cx, size - 1));

                for byte in id_bytes.into_iter() {
                    id = id << 8;
                    id |= byte as u32;
                }

                self.stack.push(id.into());
                self.parser_state = ParserStateMachine::ReadElementSizeLength();
                return self.poll_next(cx);
            }
            ParserStateMachine::ReadElementSizeLength() => {
                let first_byte = ready_next!(self.read_exact_bytes(cx, 1))[0];

                let size = first_byte.leading_zeros() as usize + 1;
                self.parser_state = ParserStateMachine::ReadElementSize(size, first_byte as u64);
                return self.poll_next(cx);
            }
            ParserStateMachine::ReadElementSize(size_of_vint, mut element_size) => {
                let size_bytes = ready_next!(self.read_exact_bytes(cx, size_of_vint - 1));

                let mask = (1 << 8 - size_of_vint) - 1;
                element_size &= mask;
                for byte in size_bytes.into_iter() {
                    element_size = element_size << 8;
                    element_size |= byte as u64
                }

                self.parser_state = ParserStateMachine::ReadElementData(element_size as usize);
                return self.poll_next(cx);
            }
            ParserStateMachine::ReadElementData(element_size) => {
                let current_element = self.stack.last().unwrap();

                match current_element {
                    EbmlElementId::Header => {
                        self.parser_state = ParserStateMachine::ReadElementIdLength;
                    }
                    EbmlElementId::DocType => {
                        let data = ready_next!(self.read_exact_bytes(cx, element_size));
                        let webm_string = match String::from_utf8(data) {
                            Ok(s) => s,
                            Err(e) => {
                                error!("Unexpected DocType: {}", e);
                                return Poll::Pending;
                            }
                        };
                        if webm_string != "webm" {
                            error!("Expected DocType webm, got: {}", webm_string);
                        }
                        self.stack.pop();
                        self.parser_state = ParserStateMachine::ReadElementIdLength;
                    }
                    EbmlElementId::Segment => {
                        self.parser_state = ParserStateMachine::ReadElementIdLength;
                    }
                    EbmlElementId::Cluster => {
                        self.parser_state = ParserStateMachine::ReadElementIdLength;
                    }
                    EbmlElementId::SimpleBlock => {
                        let mut data = ready_next!(self.read_exact_bytes(cx, element_size));
                        // TODO: parse vint, make sure block corresponds to opus track
                        self.stack.pop();
                        self.parser_state = ParserStateMachine::ReadElementIdLength;
                        self.simple_blocks += 1;
                        trace!("Num simple blocks: {}", self.simple_blocks);
                        return Poll::Ready(Some(data.split_off(4)));
                    }
                    _ => {
                        if !self.seek_in_progress {
                            let start_seek = Pin::new(&mut self.stream)
                                .start_seek(SeekFrom::Current(element_size as i64));
                            if let Err(e) = start_seek {
                                error!("Error with seeking {} position ahead: {}", element_size, e);
                                return Poll::Ready(None);
                            }
                            self.seek_in_progress = true;
                        }
                        if let Poll::Ready(new_pos) = Pin::new(&mut self.stream).poll_complete(cx) {
                            trace!("New Position: {}", new_pos.unwrap());
                        } else {
                            return Poll::Pending;
                        }
                        self.stack.pop();
                        self.seek_in_progress = false;
                        self.parser_state = ParserStateMachine::ReadElementIdLength;
                    }
                }
                return self.poll_next(cx);
            }
        }
    }
}

impl<R: AsyncRead + AsyncSeek + Unpin> WebmStream<BufReader<R>> {
    pub fn new(stream: R) -> Self {
        WebmStream {
            stream: BufReader::new(stream),
            stack: Vec::new(),
            parser_state: ParserStateMachine::ReadElementIdLength,
            buf: None,
            cursor: 0,
            seek_in_progress: false,
            simple_blocks: 0,
        }
    }
}
