pub struct ReadAheadBuf {
    data: Vec<u8>,
    buf_offset: u64,
    max_size: usize,
    last_read_end: u64,
    sequential_count: u32,
}

impl ReadAheadBuf {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: Vec::new(),
            buf_offset: 0,
            max_size,
            last_read_end: u64::MAX,
            sequential_count: 0,
        }
    }

    pub fn try_read(&mut self, offset: u64, len: usize) -> Option<Vec<u8>> {
        if offset == self.last_read_end {
            self.sequential_count = self.sequential_count.saturating_add(1);
        } else {
            self.sequential_count = 0;
        }

        let buf_end = self.buf_offset + self.data.len() as u64;
        if offset >= self.buf_offset && offset + len as u64 <= buf_end {
            let start = (offset - self.buf_offset) as usize;
            let result = self.data[start..start + len].to_vec();
            self.last_read_end = offset + len as u64;
            return Some(result);
        }

        None
    }

    pub fn fill(&mut self, offset: u64, data: Vec<u8>) {
        self.buf_offset = offset;
        self.data = data;
    }

    pub fn fetch_plan(&self, offset: u64, requested_len: usize) -> (u64, usize) {
        if self.sequential_count >= 2 {
            (offset, self.max_size.min(requested_len * 4))
        } else {
            (offset, requested_len)
        }
    }

    pub fn invalidate(&mut self) {
        self.data.clear();
        self.buf_offset = 0;
    }

    pub fn is_sequential(&self) -> bool {
        self.sequential_count >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_hit() {
        let mut buf = ReadAheadBuf::new(4096);
        buf.fill(0, vec![10, 20, 30, 40, 50]);

        let result = buf.try_read(1, 3).unwrap();
        assert_eq!(result, vec![20, 30, 40]);
    }

    #[test]
    fn buffer_miss() {
        let mut buf = ReadAheadBuf::new(4096);
        buf.fill(0, vec![10, 20, 30]);

        // Read past the end of the buffer.
        assert!(buf.try_read(0, 5).is_none());
        // Read before the buffer start (buffer starts at 0, but offset
        // 100 is not covered).
        assert!(buf.try_read(100, 1).is_none());
    }

    #[test]
    fn sequential_detection() {
        let mut buf = ReadAheadBuf::new(4096);
        buf.fill(0, vec![0; 300]);

        // First read at offset 0, length 100 -- not sequential yet.
        buf.try_read(0, 100);
        assert!(!buf.is_sequential());

        // Second read continues where the first left off.
        buf.try_read(100, 100);
        assert!(!buf.is_sequential());

        // Third consecutive sequential read crosses the threshold.
        buf.try_read(200, 100);
        assert!(buf.is_sequential());
    }

    #[test]
    fn fetch_plan_grows() {
        let mut buf = ReadAheadBuf::new(4096);
        buf.fill(0, vec![0; 300]);

        // Before sequential detection kicks in, plan matches request.
        let (off, len) = buf.fetch_plan(0, 100);
        assert_eq!(off, 0);
        assert_eq!(len, 100);

        // Trigger sequential detection.
        buf.try_read(0, 100);
        buf.try_read(100, 100);
        buf.try_read(200, 100);

        // Now the plan should expand the read size.
        let (off, len) = buf.fetch_plan(300, 100);
        assert_eq!(off, 300);
        assert_eq!(len, 400); // 100 * 4
    }

    #[test]
    fn invalidate_clears_buffer() {
        let mut buf = ReadAheadBuf::new(4096);
        buf.fill(0, vec![1, 2, 3]);
        assert!(buf.try_read(0, 1).is_some());

        buf.invalidate();
        assert!(buf.try_read(0, 1).is_none());
    }
}
