fn reduce_intersect<T: Eq + Clone>(sets: &[Vec<T>]) -> Vec<T> {
    let Some((first, rest)) = sets.split_first() else {
        return Vec::new();
    };
    let mut out: Vec<T> = Vec::new();
    for v in first {
        if !out.contains(v) && rest.iter().all(|s| s.contains(v)) {
            out.push(v.clone());
        }
    }
    out
}

fn main() {
    let a = vec!["USA", "USA", "GBR"];
    let b = vec!["USA", "GBR", "CAN"];
    println!("{:?}", reduce_intersect(&[a, b]));
}
