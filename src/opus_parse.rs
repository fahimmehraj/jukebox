use std::{
    borrow::BorrowMut,
    iter::Peekable,
    pin::Pin,
    task::{Context, Poll},
    vec,
};

use log::{debug, error, info, trace, warn};
use packed_struct::prelude::*;
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio_stream::Stream;

#[derive(PackedStruct, Debug, Copy, Clone)]
#[packed_struct(endian = "lsb", bit_numbering = "msb0", size_bytes = "27")]
pub struct OggPageHeader {
    #[packed_field(bytes = "0..=3")]
    capture_pattern: [u8; 4],

    #[packed_field(bytes = "4")]
    pad_byte: u8,

    #[packed_field(bytes = "5")]
    flag: u8,

    #[packed_field(bytes = "6..=13")]
    gran_pos: u64,

    #[packed_field(bytes = "14..=17")]
    serial: u32,

    #[packed_field(bytes = "18..=21")]
    pagenum: u32,

    #[packed_field(bytes = "22..=25")]
    crc: u32,

    #[packed_field(bytes = "26")]
    segnum: u8,
}

pub struct OggStream<T: AsyncReadExt + Unpin> {
    stream: T,
    segment_table: Option<Peekable<vec::IntoIter<u8>>>,
    buf: Option<Vec<u8>>,
    cursor: usize,
    read_mode: ReadMode,
    current_page_header: Option<OggPageHeader>,
    test_counter: u64,
}

enum ReadMode {
    Header,
    Segtable,
    Packet,
}

impl<T: AsyncReadExt + Unpin> Stream for OggStream<T> {
    type Item = Vec<u8>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Vec<u8>>> {
        match self.read_mode {
            ReadMode::Header => {
                let mut buf = self.buf.take().unwrap_or(vec![0u8; 27]);
                let mut read_buf = ReadBuf::new(&mut buf);
                if read_buf.capacity() == 0 {
                    return Poll::Ready(None);
                }
                read_buf.advance(self.cursor);
                // read until buf is full
                match Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
                    Poll::Ready(Ok(())) => {
                        // detect if buffer is unchanged
                        if read_buf.filled().len() == 0 {
                            return Poll::Ready(None);
                        }
                        debug!("filled portion: {:?}", read_buf.filled());
                        if read_buf.filled().len() < read_buf.capacity() {
                            debug!("asdasd: filled portion: {:?}", read_buf.filled());
                            self.cursor = read_buf.filled().len();
                            self.buf = Some(buf);
                            return Poll::Pending;
                        }
                        let header = OggPageHeader::unpack(&buf.try_into().unwrap())
                            .expect("Invalid header");
                        debug!("{:?}", &header);
                        self.current_page_header = Some(header);
                        self.read_mode = ReadMode::Segtable;
                        self.test_counter += 1;
                        debug!("test_counter: {}", self.test_counter);
                        self.buf = None;
                        self.cursor = 0;
                        self.poll_next(cx)
                    }
                    Poll::Ready(Err(e)) => {
                        error!("error reading header: {}", e);
                        Poll::Ready(None)
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
            ReadMode::Segtable => {
                let mut buf = self.buf.take().unwrap_or(vec![
                    0u8;
                    self.current_page_header
                        .expect("Page header should not be None if read mode is Segtable")
                        .segnum
                        as usize
                ]);
                let mut read_buf = ReadBuf::new(&mut buf);
                match Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
                    Poll::Ready(Ok(())) => {
                        debug!(
                            "1: filled portion: {:?}, capacity: {:?}",
                            read_buf.filled(),
                            read_buf.capacity()
                        );
                        if read_buf.filled().len() < read_buf.capacity() {
                            self.cursor = read_buf.filled().len();
                            self.buf = Some(buf);
                            return Poll::Pending;
                        }
                        self.segment_table = Some(buf.into_iter().peekable());
                        debug!("segtable: {:?}", self.segment_table.as_ref().unwrap());
                        self.read_mode = ReadMode::Packet;
                        self.buf = None;
                        self.cursor = 0;
                        self.poll_next(cx)
                    }
                    Poll::Ready(Err(e)) => {
                        error!("error reading segtable: {}", e);
                        Poll::Ready(None)
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
            ReadMode::Packet => match self
                .segment_table
                .as_mut()
                .expect("segment_table should not be None if reading mode is Packet")
                .peek()
                .copied()
            {
                Some(seg) => {
                    let mut buf = self.buf.take().unwrap_or(vec![0u8; seg as usize]);
                    if self.cursor > 0 {
                        buf.extend_from_slice(&vec![0u8; seg as usize - self.cursor]);
                    }
                    let mut read_buf = ReadBuf::new(&mut buf);
                    // read_buf.advance(self.cursor);
                    match Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
                        Poll::Ready(Ok(())) => {
                            debug!("Current segment: {}", seg);
                            if read_buf.filled().len() < read_buf.capacity() {
                                info!("Had to read more than once");
                                debug!(
                                    "2: filled portion: {:?}, capacity: {:?}",
                                    read_buf.filled(),
                                    read_buf.capacity()
                                );
                                self.cursor = read_buf.filled().len();
                                self.buf = Some(buf);
                                return Poll::Pending;
                            }

                            self.segment_table.as_mut().unwrap().next();
                            if seg == 255 {
                                return self.poll_next(cx);
                            }
                            self.cursor = 0;
                            Poll::Ready(Some(buf))
                        }
                        Poll::Ready(Err(e)) => {
                            error!("error reading packet: {}", e);
                            return Poll::Ready(None);
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }
                None => {
                    debug!("el oh el");
                    self.segment_table = None;
                    self.read_mode = ReadMode::Header;
                    self.buf = None;
                    self.poll_next(cx)
                }
            },
        }
    }
}

impl<T: AsyncRead + Unpin> OggStream<T> {
    pub fn new(stream: T) -> Self {
        OggStream {
            stream,
            segment_table: None,
            current_page_header: None,
            buf: None,
            cursor: 0,
            read_mode: ReadMode::Header,
            test_counter: 0,
        }
    }
}
