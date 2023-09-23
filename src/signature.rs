use crate::commands::CommandError;
 
  use crate::pb_signature::signature::Hash;
use crate::pb_signature::Signature;
use protobuf::Message;
use sha2::Digest;
use sha2::Sha256;
use std::fs::File;

use std::io::Read;
use std::path::Path;

pub fn sig_to_hex(signature: &Signature) -> String {
    let serialized_bytes = signature
        .write_to_bytes()
        .expect("Failed to serialize the message.");

    hex::encode(serialized_bytes)
}

const CHUNK_SIZE: usize = 512 * 1024;

pub fn generate_signature(path: &Path) -> Result<Signature, CommandError> {
    let mut file = File::open(path)?;
    let mut fullhash = Sha256::new();
    let mut hashes_hash = Sha256::new();
    let mut signature = Signature::new();
    let mut pos: u64 = 0;
    let mut chunk = vec![0; CHUNK_SIZE];

    loop {
        let bytes_read = file.read(&mut chunk)?;
        let chunk_slice = &chunk[..bytes_read];

        if chunk_slice.is_empty() {
            break;
        }

        let mut hash_entry = Hash::new();
        hash_entry.set_offset(pos as i64);
        hash_entry.set_hash(checksum(chunk_slice));
        signature.hashes.push(hash_entry.clone());
        fullhash.update(chunk_slice);
        hashes_hash.update(hash_entry.hash());

        pos += bytes_read as u64;
    }

    signature.set_full_hash(fullhash.finalize().to_vec());
    signature.set_hashes_hash(hashes_hash.finalize().to_vec());

    Ok(signature)
}

pub fn checksum(chunk: &[u8]) -> Vec<u8> {
    let hash_result = sha1::Sha1::digest(chunk);
    hash_result.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_generate_signature_when_file_does_not_exist_should_return_error() {
        let file_path = Path::new("/non-existent-file");
        let result = generate_signature(file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_signature_should_generate_correct_signature_for_file() {
        let tmp_dir = tempdir().unwrap();
        let file_content = "test test test test";
        fs::write(
            tmp_dir.path().join("index.html").to_str().unwrap(),
            file_content,
        )
        .unwrap();
        let signature = generate_signature(tmp_dir.path().join("index.html").as_path());
        assert!(signature.is_ok());
        assert_eq!(sig_to_hex(&signature.unwrap()),
                   "0a20098fd57f8a1c688e437e0309a745173409029a6a8038385572e4187a2f7570501220b24375dfa261c3cb47029f5dd0dc00bd081dffc440916c1ff45e9ff0144a61761a180a14230c1b958ba91ab37a68f965818b8d74a8b171fb1000")
    }
}
