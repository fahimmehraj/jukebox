# jukebox
Audio sending server that will cover freyacodes/Lavalink's forward facing API

## Why?
The Java Garbage Collector was too much of a nuisance for certain embedded systems that I wanted to send audio from. It made sense to make a new audio sending node from scratch using a more performant language.

## Goals
- [ ] Replicate lavalink's public endpoints (websocket and rest operations)