use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::RpcError;

/// Hard limit on record size to prevent unbounded memory allocation
/// from malicious or misbehaving peers. NFSv4.1 max compound size is
/// typically well under 1 MB, so 10 MB is generous.
pub const MAX_RECORD_SIZE: usize = 10 * 1024 * 1024;

/// Bit 31 of the fragment header indicates the last fragment in a record.
const LAST_FRAGMENT: u32 = 0x8000_0000;

/// Result of attempting to read a record from a stream of bytes.
#[derive(Debug)]
pub enum RecordResult {
    /// A complete record has been reassembled from one or more fragments.
    Complete(Bytes),
    /// More data is needed before a complete record can be returned.
    Incomplete,
}

/// Writes a complete record as a single fragment into `buf`.
///
/// Prepends a 4-byte fragment header with the last-fragment flag set
/// and the length of `data`.
pub fn write_record(data: &[u8], buf: &mut BytesMut) {
    let header = LAST_FRAGMENT | data.len() as u32;
    buf.put_u32(header);
    buf.put_slice(data);
}

/// Reassembles ONC RPC records from arbitrarily chunked input.
///
/// RFC 5531 section 11 defines record marking: each record consists of
/// one or more fragments, each preceded by a 4-byte header. Bit 31 of
/// the header is the last-fragment flag; bits 0-30 give the fragment
/// length.
#[derive(Debug)]
pub struct RecordReader {
    /// Raw bytes received but not yet consumed.
    input: BytesMut,
    /// Accumulated payload bytes for the current record.
    record: BytesMut,
    /// Bytes remaining in the current fragment.
    remaining: usize,
    /// Whether we are in the middle of reading a fragment body.
    in_fragment: bool,
    /// Whether the current fragment is the last in its record.
    is_last: bool,
}

impl RecordReader {
    pub fn new() -> Self {
        Self {
            input: BytesMut::new(),
            record: BytesMut::new(),
            remaining: 0,
            in_fragment: false,
            is_last: false,
        }
    }

    /// Feed `data` to the reader and attempt to produce a complete record.
    ///
    /// Returns `RecordResult::Complete` when a full record has been
    /// reassembled, or `RecordResult::Incomplete` when more data is
    /// needed. Leftover bytes after a complete record are retained for
    /// subsequent calls.
    pub fn read(&mut self, data: &[u8]) -> Result<RecordResult, RpcError> {
        self.input.extend_from_slice(data);

        loop {
            if !self.in_fragment {
                // Need 4 bytes for the fragment header.
                if self.input.len() < 4 {
                    return Ok(RecordResult::Incomplete);
                }

                let header = self.input.get_u32();
                self.is_last = (header & LAST_FRAGMENT) != 0;
                let length = (header & !LAST_FRAGMENT) as usize;

                if self.record.len() + length > MAX_RECORD_SIZE {
                    return Err(RpcError::RecordTooLarge {
                        size: self.record.len() + length,
                        max: MAX_RECORD_SIZE,
                    });
                }

                self.remaining = length;
                self.in_fragment = true;
            }

            // Consume as many bytes as possible from the current fragment.
            let available = self.input.len().min(self.remaining);
            if available > 0 {
                self.record.extend_from_slice(&self.input[..available]);
                self.input.advance(available);
                self.remaining -= available;
            }

            if self.remaining == 0 {
                // Fragment is complete.
                self.in_fragment = false;

                if self.is_last {
                    let record = self.record.split().freeze();
                    return Ok(RecordResult::Complete(record));
                }
                // Not the last fragment — loop to read the next header.
            } else {
                // Need more data to finish this fragment.
                return Ok(RecordResult::Incomplete);
            }
        }
    }
}

impl Default for RecordReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_single_record() {
        let payload = b"hello";
        let mut buf = BytesMut::new();
        write_record(payload, &mut buf);

        // Total length: 4-byte header + 5-byte payload.
        assert_eq!(buf.len(), 9);

        // Header: last-fragment flag | 5.
        let header = u32::from_be_bytes(buf[..4].try_into().unwrap());
        assert_eq!(header, LAST_FRAGMENT | 5);

        // Payload follows header.
        assert_eq!(&buf[4..], b"hello");
    }

    #[test]
    fn read_single_fragment_complete() {
        let payload = b"world";
        let mut buf = BytesMut::new();
        write_record(payload, &mut buf);

        let mut reader = RecordReader::new();
        let result = reader.read(&buf).unwrap();

        match result {
            RecordResult::Complete(data) => assert_eq!(&data[..], b"world"),
            RecordResult::Incomplete => panic!("expected Complete"),
        }
    }

    #[test]
    fn read_incremental_bytes() {
        let payload = b"abc";
        let mut buf = BytesMut::new();
        write_record(payload, &mut buf);

        let mut reader = RecordReader::new();

        // Feed one byte at a time; all but the last should be Incomplete.
        for i in 0..buf.len() - 1 {
            let result = reader.read(&buf[i..i + 1]).unwrap();
            assert!(
                matches!(result, RecordResult::Incomplete),
                "expected Incomplete at byte {i}"
            );
        }

        // The last byte should complete the record.
        let last = buf.len() - 1;
        let result = reader.read(&buf[last..last + 1]).unwrap();
        match result {
            RecordResult::Complete(data) => assert_eq!(&data[..], b"abc"),
            RecordResult::Incomplete => panic!("expected Complete on final byte"),
        }
    }

    #[test]
    fn read_multiple_fragments() {
        // Build a record consisting of three fragments:
        // fragment 1 (not last): b"aa"
        // fragment 2 (not last): b"bb"
        // fragment 3 (last):     b"cc"
        let mut wire = BytesMut::new();

        // Non-last fragment: length without LAST_FRAGMENT flag.
        wire.put_u32(2); // not last, length=2
        wire.put_slice(b"aa");

        wire.put_u32(2); // not last, length=2
        wire.put_slice(b"bb");

        wire.put_u32(LAST_FRAGMENT | 2); // last, length=2
        wire.put_slice(b"cc");

        let mut reader = RecordReader::new();
        let result = reader.read(&wire).unwrap();
        match result {
            RecordResult::Complete(data) => assert_eq!(&data[..], b"aabbcc"),
            RecordResult::Incomplete => panic!("expected Complete"),
        }
    }

    #[test]
    fn empty_record() {
        let mut buf = BytesMut::new();
        write_record(b"", &mut buf);

        let mut reader = RecordReader::new();
        let result = reader.read(&buf).unwrap();
        match result {
            RecordResult::Complete(data) => assert!(data.is_empty()),
            RecordResult::Incomplete => panic!("expected Complete for empty record"),
        }
    }

    #[test]
    fn record_too_large() {
        // Craft a fragment header claiming a size that exceeds the limit.
        let mut wire = BytesMut::new();
        let oversized = (MAX_RECORD_SIZE + 1) as u32;
        wire.put_u32(LAST_FRAGMENT | oversized);

        let mut reader = RecordReader::new();
        let result = reader.read(&wire);
        assert!(result.is_err());
        match result.unwrap_err() {
            RpcError::RecordTooLarge { size, max } => {
                assert_eq!(size, MAX_RECORD_SIZE + 1);
                assert_eq!(max, MAX_RECORD_SIZE);
            }
            other => panic!("expected RecordTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn consecutive_records() {
        // Write two records back-to-back into a single buffer.
        let mut wire = BytesMut::new();
        write_record(b"first", &mut wire);
        write_record(b"second", &mut wire);

        let mut reader = RecordReader::new();

        // First read should return the first record.
        let result = reader.read(&wire).unwrap();
        match result {
            RecordResult::Complete(data) => assert_eq!(&data[..], b"first"),
            RecordResult::Incomplete => panic!("expected first Complete"),
        }

        // Second record is already buffered; read with empty input.
        let result = reader.read(&[]).unwrap();
        match result {
            RecordResult::Complete(data) => assert_eq!(&data[..], b"second"),
            RecordResult::Incomplete => panic!("expected second Complete"),
        }
    }
}
