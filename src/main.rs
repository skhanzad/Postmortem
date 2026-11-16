mod git;



fn main() {
    let repo = gix::discover(".").unwrap();
    println!("{}", git::diff(&repo).unwrap());
}