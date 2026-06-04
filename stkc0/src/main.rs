use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;
use stkc0::{compress, decompress};
use flate2::read::GzEncoder;
use flate2::Compression;

fn bench_codec<F: Fn(&[u8]) -> Vec<u8>, G: Fn(&[u8]) -> Vec<u8>>(
    label: &str, data: &[u8], enc: F, dec: G,
) {
    let t0 = Instant::now();
    let c = enc(data);
    let et = t0.elapsed();
    let t1 = Instant::now();
    let dec_data = dec(&c);
    let dt = t1.elapsed();
    let ok = if dec_data == data { "OK" } else { "FAIL" };
    let pct = 100.0 * c.len() as f64 / data.len() as f64;
    println!("{:<20} {:>6}B -> {:>6}B ({:>5.1}%) enc:{:>8.1}ms dec:{:>6.1}ms [{}]",
             label, data.len(), c.len(), pct,
             et.as_secs_f64() * 1000.0, dt.as_secs_f64() * 1000.0, ok);
}

fn gzip_enc(data: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut enc = GzEncoder::new(data, Compression::best());
    let mut out = Vec::new();
    enc.read_to_end(&mut out).unwrap();
    out
}

fn gzip_dec(data: &[u8]) -> Vec<u8> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut dec = GzDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).unwrap();
    out
}

fn zstd_enc(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    zstd::stream::copy_encode(data, &mut out, 19).unwrap();
    out
}

fn zstd_dec(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    zstd::stream::copy_decode(data, &mut out).unwrap();
    out
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("stkc0: Scan-based LZ77 compressor (auto-picks 3-byte/4-byte hash)");
        eprintln!("Usage:");
        eprintln!("  stkc0 c <input> [output]   Compress");
        eprintln!("  stkc0 d <input> [output]   Decompress");
        eprintln!("  stkc0 t <input>            Test round-trip");
        eprintln!("  stkc0 bench                Run benchmarks");
        eprintln!("  stkc0 bench-corpus         Run Calgary/Canterbury corpus benchmarks");
        return;
    }

    match args[1].as_str() {
        "c" => {
            let data = fs::read(&args[2]).expect("read input");
            let t0 = Instant::now();
            let out = compress(&data);
            let elapsed = t0.elapsed();
            let dest = if args.len() > 3 { args[3].clone() }
                        else { format!("{}.stkc0", args[2]) };
            fs::write(&dest, &out).expect("write output");
            let pct = 100.0 * out.len() as f64 / data.len() as f64;
            eprintln!("{} -> {} ({:.1}%, {:.1}ms)", args[2], dest, pct,
                      elapsed.as_secs_f64() * 1000.0);
        }
        "d" => {
            let data = fs::read(&args[2]).expect("read input");
            let t0 = Instant::now();
            let out = decompress(&data);
            let elapsed = t0.elapsed();
            let dest = if args.len() > 3 { args[3].clone() }
                        else { args[2].trim_end_matches(".stkc0").to_string() };
            fs::write(&dest, &out).expect("write output");
            eprintln!("{} -> {} ({}B, {:.1}ms)", args[2], dest, out.len(),
                      elapsed.as_secs_f64() * 1000.0);
        }
        "t" => {
            let data = fs::read(&args[2]).expect("read input");
            let t0 = Instant::now();
            let c = compress(&data);
            let ct = t0.elapsed();
            let t1 = Instant::now();
            let dec = decompress(&c);
            let dt = t1.elapsed();
            let ok = if dec == data { "OK" } else { "FAIL" };
            let pct = 100.0 * c.len() as f64 / data.len() as f64;
            eprintln!("{}: {}B -> {}B ({:.1}%) enc:{:.1}ms dec:{:.1}ms [{}]",
                      args[2], data.len(), c.len(), pct,
                      ct.as_secs_f64() * 1000.0,
                      dt.as_secs_f64() * 1000.0, ok);
        }
        "bench-corpus" => run_bench_corpus(),
        _ => run_bench(),
    }
}

fn run_bench() {
    use std::f64::consts::PI;

    let text = b"Hello World! Hello World! hELLO wORLD!";
    let base: Vec<u8> = (0..=255).collect();
    let shifted: Vec<u8> = base.iter().map(|&b| b.wrapping_add(10)).collect();
    let subbed: Vec<u8> = base.iter().map(|&b| b.wrapping_sub(10)).collect();
    let repeated: Vec<u8> = b"abcdefghijklmnop".repeat(500);
    let pat: Vec<u8> = vec![0x10, 0x20, 0x30, 0x40, 0x30, 0x40, 0x50, 0x60, 0x90, 0xA0, 0xB0, 0xC0];
    let mixed: Vec<u8> = pat.repeat(100);

    fn lcg(seed: &mut u64) -> u8 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*seed >> 32) as u8
    }
    let mut seed: u64 = 42;
    let rand_data: Vec<u8> = (0..10000).map(|_| lcg(&mut seed)).collect();

    let sine: Vec<u8> = (0..4096)
        .map(|i| (128.0 + 100.0 * (2.0 * PI * i as f64 / 64.0).sin()).round() as u8)
        .collect();

    let all_base: Vec<u8> = base.iter().chain(&shifted).copied().collect();
    let all_sub: Vec<u8> = base.iter().chain(&subbed).copied().collect();

    // real files
    let real_files: [(&str, &str); 15] = [
        ("tlz3.py",          r"..\pawnet\tlz3.py"),
        ("rans.rs",          r"..\tkc1\src\rans.rs"),
        ("tiktok.py",        r"C:\Users\komori\AppData\Local\Alexa\Virtual\Lib\site-packages\youtube_dl\extractor\tiktok.py"),
        ("LICENSE.txt",      r"C:\Users\komori\AppData\Local\Alexa\Virtual\LICENSE.txt"),
        ("README.txt",       r"C:\Users\komori\AppData\Local\Alexa\Virtual\README.txt"),
        ("app.py",           r"..\..\..\Desktop\komori_portfolio\app.py"),
        ("pawnet.html",      r"..\tkc1\test_data\pawnet_monitor.html"),
        ("spine64_README",   r"..\tkc1\test_data\README.md"),
        ("spine64_SPEC",     r"..\tkc1\test_data\SPEC.md"),
        ("chat_sitemap.xml", r"..\tkc1\test_data\chat_sitemap.xml"),
        ("chat_rules.txt",   r"..\tkc1\test_data\chat_rules.txt"),
        ("chat_messages.txt",r"..\tkc1\test_data\chat_messages.txt"),
        ("chat_cli_code.txt",r"..\tkc1\test_data\chat_cli_code.txt"),
        ("chat_pages.txt",   r"..\tkc1\test_data\chat_pages.txt"),
        ("ddlc_log.txt",     r"..\tkc1\test_data\ddlc_log.txt"),
    ];

    let tests: [(&str, &[u8]); 7] = [
        ("XOR text", text),
        ("ADD shift", &all_base),
        ("SUB shift", &all_sub),
        ("mixed", &mixed),
        ("exact", &repeated),
        ("sine", &sine),
        ("random10k", &rand_data),
    ];

    // load real files
    let mut real_data: Vec<(&str, Vec<u8>)> = Vec::new();
    for (label, path) in &real_files {
        match std::fs::read(path) {
            Ok(d) => { real_data.push((label, d)); }
            Err(e) => { eprintln!("  skipping {}: {}", label, e); }
        }
    }

    let codecs: [(&str, fn(&[u8]) -> Vec<u8>, fn(&[u8]) -> Vec<u8>); 3] = [
        ("--- stkc0 ---", compress, decompress),
        ("--- gzip -9 ---", gzip_enc, gzip_dec),
        ("--- zstd -19 ---", zstd_enc, zstd_dec),
    ];

    for (header, enc, dec) in &codecs {
        println!("{}", header);
        for (label, data) in &tests {
            bench_codec(label, data, *enc, *dec);
        }
    }

    // real file benchmarks (stkc0, gzip, zstd)
    println!("\n--- real files (stkc0) ---");
    for (label, data) in &real_data {
        bench_codec(label, data, compress, decompress);
    }
    println!("--- real files (zstd -19) ---");
    for (label, data) in &real_data {
        bench_codec(label, data, zstd_enc, zstd_dec);
    }
    println!("--- real files (gzip -9) ---");
    for (label, data) in &real_data {
        bench_codec(label, data, gzip_enc, gzip_dec);
    }
}

fn run_bench_corpus() {
    let test_dir = Path::new("test_data");
    let mut corpus_data: Vec<(String, Vec<u8>)> = Vec::new();

    if let Ok(entries) = fs::read_dir(test_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap().to_str().unwrap().to_string();
                if name == "README.md" { continue; }
                let label = name.trim_start_matches("calgary_")
                               .trim_start_matches("canterbury_")
                               .trim_start_matches("misc_")
                               .to_string();
                if let Ok(data) = fs::read(&path) {
                    corpus_data.push((label, data));
                }
            }
        }
    }

    corpus_data.sort_by(|a, b| a.0.cmp(&b.0));
    let corpus: Vec<(&str, &[u8])> = corpus_data.iter().map(|(l, d)| (l.as_str(), d.as_slice())).collect();

    println!("--- stkc0 ---");
    for (label, data) in &corpus {
        bench_codec(label, data, compress, decompress);
    }
    println!("\n--- gzip -9 ---");
    for (label, data) in &corpus {
        bench_codec(label, data, gzip_enc, gzip_dec);
    }
    println!("\n--- diff (stkc0 - gzip) ---");
    for (label, data) in &corpus {
        let tkc = compress(data);
        let gz = gzip_enc(data);
        let diff = (tkc.len() as i64) - (gz.len() as i64);
        let sign = if diff > 0 { "+" } else { "" };
        let gz_pct = 100.0 * gz.len() as f64 / data.len() as f64;
        let tkc_pct = 100.0 * tkc.len() as f64 / data.len() as f64;
        println!("{:<20} stkc0:{:>6}B ({:>5.1}%)  gzip:{:>6}B ({:>5.1}%)  diff:{}{}B",
                 label, tkc.len(), tkc_pct, gz.len(), gz_pct, sign, diff);
    }
}
