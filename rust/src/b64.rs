//! Base64 标准字母表（RFC 4648，带 padding）的零依赖编解码。
//!
//! 分叉的附件链路两端都要它：发送侧把规范化后的图片字节挂进 content 数组
//! （`data` 字段，主干 `Convert.ToBase64String`），回读侧把 `session/attachment`
//! 的 `data:base64` 解回字节。依赖清单（Cargo.toml）刻意保持极简，这 60 行不值得
//! 为它引 `base64` crate——`nativepick`（comdlg32 裸 FFI）走的同一条零依赖仓规。

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// 标准编码，带 `=` padding（内核 JSON 与主干同款）。
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// 标准解码；`=` padding 收尾即停。坏字符回 `None`（调用方降级，不炸消息流）。
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for &c in text.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **L-B641**：RFC 4648 的官方测试向量逐条对齐（含 padding 两档）。
    #[test]
    fn rfc4648_vectors() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    /// **L-B642**：decode 与 encode 互逆；坏字符 None；`=` 后停。
    #[test]
    fn roundtrip_and_rejects() {
        let bytes: Vec<u8> = (0..=255u8).collect();
        assert_eq!(decode(&encode(&bytes)).as_deref(), Some(bytes.as_slice()));
        assert_eq!(decode("Zm9v").as_deref(), Some(b"foo".as_slice()));
        assert_eq!(decode("Zg=="), Some(b"f".to_vec()));
        // padding 后再有字符就不再吃（主干 Convert.FromBase64String 的宽松度不照抄，
        // 内核回读的字节是干净串）。
        assert_eq!(decode("Zg==xxx"), Some(b"f".to_vec()));
        assert_eq!(decode("Zm9*"), None);
        assert_eq!(decode(""), Some(Vec::new()));
    }
}
