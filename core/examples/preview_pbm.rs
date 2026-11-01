//! cargo run --example preview_pbm -- <label file> <out.pbm>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = std::fs::read(&a[1]).unwrap();
    let b = if dotline_core::preview::language(&p) == "zpl" { dotline_core::preview::decode_zpl(&p) } else { dotline_core::preview::decode_ezpl(&p) }
        .expect("no bitmap");
    let mut out = format!("P4\n{} {}\n", b.width_dots, b.height_dots).into_bytes();
    out.extend(&b.bytes);
    std::fs::write(&a[2], out).unwrap();
}
