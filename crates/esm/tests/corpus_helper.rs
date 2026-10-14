//! M0.5: the corpus helper reads the manifest and resolves files, or skips cleanly; its
//! SHA-256 matches the FIPS 180-4 test vectors.

mod common;

#[test]
fn manifest_lists_the_fourteen_official_esms() {
    let manifest = common::manifest();
    assert_eq!(manifest.len(), 14);
    for entry in &manifest {
        assert!(entry.name.ends_with(".esm"), "{}", entry.name);
        assert!(entry.size > 0, "{}", entry.name);
        assert_eq!(entry.sha256.len(), 64, "{}", entry.name);
    }
    for small in common::SMALL.iter().chain(&common::TINY) {
        assert!(manifest.iter().any(|e| e.name == *small), "{small}");
    }
}

#[test]
fn small_files_resolve_or_skip() {
    let Some(files) = common::files("corpus_helper", &common::SMALL) else {
        return;
    };
    assert_eq!(files.len(), common::SMALL.len());
    for file in &files {
        assert!(file.path.is_file(), "{}", file.path.display());
        assert!(file.size < 512 * 1024, "{} is not small", file.name);
        let tiny = common::TINY.contains(&file.name.as_str());
        assert_eq!(tiny, file.size < 64 * 1024, "{}", file.name);
    }
}

#[test]
fn sha256_matches_the_standard_vectors() {
    use common::sha256::hex_digest_of;
    assert_eq!(
        hex_digest_of(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        hex_digest_of(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        hex_digest_of(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    // One million 'a', fed in 7-byte reads so blocks straddle reads.
    struct Dribble<'a>(&'a [u8]);
    impl std::io::Read for Dribble<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.0.len().min(buf.len()).min(7);
            let (head, tail) = self.0.split_at(n);
            buf.get_mut(..n)
                .ok_or(std::io::ErrorKind::InvalidInput)?
                .copy_from_slice(head);
            self.0 = tail;
            Ok(n)
        }
    }
    let million = vec![b'a'; 1_000_000];
    assert_eq!(
        common::sha256::hex_digest(Dribble(&million)).unwrap_or_default(),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}
