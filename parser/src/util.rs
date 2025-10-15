use std::io::Read;

use anyhow::Result;

pub fn u1(r: &mut impl Read) -> Result<u8> {
    let mut buf = [0; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

pub fn u2(r: &mut impl Read) -> Result<u16> {
    let mut buf = [0; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}

pub fn u4(r: &mut impl Read) -> Result<u32> {
    let mut buf = [0; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

pub fn i8(r: &mut impl Read) -> Result<i64> {
    let mut buf = [0; 8];
    r.read_exact(&mut buf)?;
    Ok(i64::from_be_bytes(buf))
}

pub fn i4(r: &mut impl Read) -> Result<i32> {
    let mut buf = [0; 4];
    r.read_exact(&mut buf)?;
    Ok(i32::from_be_bytes(buf))
}

pub fn f4(r: &mut impl Read) -> Result<f32> {
    let mut buf = [0; 4];
    r.read_exact(&mut buf)?;
    Ok(f32::from_be_bytes(buf))
}

pub fn f8(r: &mut impl Read) -> Result<f64> {
    let mut buf = [0; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_be_bytes(buf))
}

pub fn utf8(r: &mut impl Read, length: usize) -> Result<String> {
    let mut buf = vec![0; length];
    r.read_exact(&mut buf)?;
    Ok(String::from_utf8(buf)?)
}

pub fn vec(r: &mut impl Read, length: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0; length];
    r.read_exact(&mut buf)?;
    Ok(buf)
}
