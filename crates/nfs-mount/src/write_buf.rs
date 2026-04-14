#[derive(Debug)]
pub struct FlushData {
    pub offset: u64,
    pub data: Vec<u8>,
}

pub struct WriteBuf {
    data: Vec<u8>,
    buf_offset: u64,
    max_size: usize,
    dirty: bool,
}

impl WriteBuf {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: Vec::new(),
            buf_offset: 0,
            max_size,
            dirty: false,
        }
    }

    pub fn write(&mut self, offset: u64, data: &[u8]) -> Option<FlushData> {
        let flush = if self.dirty {
            let buf_end = self.buf_offset + self.data.len() as u64;
            if offset != buf_end || self.data.len() + data.len() > self.max_size {
                Some(self.take_flush())
            } else {
                None
            }
        } else {
            None
        };

        if !self.dirty {
            self.buf_offset = offset;
        }
        self.data.extend_from_slice(data);
        self.dirty = true;
        flush
    }

    pub fn flush(&mut self) -> Option<FlushData> {
        if self.dirty {
            Some(self.take_flush())
        } else {
            None
        }
    }

    fn take_flush(&mut self) -> FlushData {
        let data = FlushData {
            offset: self.buf_offset,
            data: std::mem::take(&mut self.data),
        };
        self.buf_offset = 0;
        self.dirty = false;
        data
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn buffered_len(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_writes_coalesce() {
        let mut buf = WriteBuf::new(1024);

        // First write -- no flush expected.
        let flush = buf.write(0, &[1, 2, 3]);
        assert!(flush.is_none());

        // Second contiguous write -- still no flush.
        let flush = buf.write(3, &[4, 5, 6]);
        assert!(flush.is_none());

        assert_eq!(buf.buffered_len(), 6);
        assert!(buf.is_dirty());
    }

    #[test]
    fn non_contiguous_flushes() {
        let mut buf = WriteBuf::new(1024);
        buf.write(0, &[1, 2, 3]);

        // Write at a non-contiguous offset should flush the old data.
        let flush = buf.write(100, &[7, 8, 9]).unwrap();
        assert_eq!(flush.offset, 0);
        assert_eq!(flush.data, vec![1, 2, 3]);

        // The new data should be buffered.
        assert_eq!(buf.buffered_len(), 3);
    }

    #[test]
    fn buffer_full_flushes() {
        let mut buf = WriteBuf::new(4);
        buf.write(0, &[1, 2]);

        // This write would exceed max_size, so old data gets flushed.
        let flush = buf.write(2, &[3, 4, 5]).unwrap();
        assert_eq!(flush.offset, 0);
        assert_eq!(flush.data, vec![1, 2]);

        // New data is buffered starting from the new offset.
        assert_eq!(buf.buffered_len(), 3);
    }

    #[test]
    fn explicit_flush() {
        let mut buf = WriteBuf::new(1024);
        buf.write(10, &[1, 2, 3]);

        let flush = buf.flush().unwrap();
        assert_eq!(flush.offset, 10);
        assert_eq!(flush.data, vec![1, 2, 3]);
        assert!(!buf.is_dirty());
    }

    #[test]
    fn empty_flush_returns_none() {
        let mut buf = WriteBuf::new(1024);
        assert!(buf.flush().is_none());
    }
}
