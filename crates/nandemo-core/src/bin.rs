//! リトルエンディアンのバイナリを読むための小さなリーダー。

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct ReadError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}（オフセット {}）", self.message, self.offset)
    }
}

impl std::error::Error for ReadError {}

pub type Result<T> = std::result::Result<T, ReadError>;

pub struct BinReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn error(&self, message: impl Into<String>) -> ReadError {
        ReadError {
            offset: self.pos,
            message: message.into(),
        }
    }

    pub fn bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        if len > self.remaining() {
            return Err(self.error(format!("{len} バイト読もうとしましたがデータが足りません")));
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    pub fn skip(&mut self, len: usize) -> Result<()> {
        self.bytes(len).map(|_| ())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.bytes(N)?.try_into().unwrap())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    pub fn f32s<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut values = [0.0; N];
        for v in &mut values {
            *v = self.f32()?;
        }
        Ok(values)
    }

    /// 要素数を読みます。壊れたファイルで巨大な確保をしないよう、
    /// 1 要素あたり `min_size` バイト以上あるとみなして残りサイズと照合します。
    pub fn count(&mut self, min_size: usize) -> Result<usize> {
        let n = self.i32()?;
        if n < 0 || (n as usize).saturating_mul(min_size) > self.remaining() {
            return Err(self.error(format!("要素数 {n} が不正です")));
        }
        Ok(n as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_values_and_reports_eof() {
        let data = [1, 0xff, 0xff, 0, 0, 0x80, 0x3f];
        let mut r = BinReader::new(&data);
        assert_eq!(r.u8().unwrap(), 1);
        assert_eq!(r.i16().unwrap(), -1);
        assert_eq!(r.f32().unwrap(), 1.0);
        let err = r.u8().unwrap_err();
        assert_eq!(err.offset, 7);
    }

    #[test]
    fn rejects_huge_count() {
        let data = 1000i32.to_le_bytes();
        assert!(BinReader::new(&data).count(1).is_err());
    }
}
