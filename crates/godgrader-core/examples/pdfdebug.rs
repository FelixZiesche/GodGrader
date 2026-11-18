fn main() {
    let path = std::env::args().nth(1).unwrap();
    let pi: usize = std::env::args().nth(2).unwrap().parse().unwrap();
    let pages = godgrader_core::ewh::pdf::extract(&std::fs::read(path).unwrap()).unwrap();
    let p = &pages[pi];
    for r in &p.rects {
        if r.y0 < 300.0 {
            println!(
                "rect {:.1} {:.1} {:.1} {:.1} g{:.2}",
                r.x0, r.y0, r.x1, r.y1, r.gray
            );
        }
    }
    for c in &p.chars {
        if c.y > 145.0 && c.y < 170.0 {
            println!("char {:?} {:.1} {:.1} s{:.1}", c.ch, c.x0, c.y, c.size);
        }
    }
}
