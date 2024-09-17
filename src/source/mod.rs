mod zbkyyy;
mod ijujitv;

pub use zbkyyy::ZBKYYY;
pub use zbkyyy::SearchResult;
pub use ijujitv::IJUJITV;

pub trait FilmInfo {
    fn name(&self) -> &str;
    fn release_time(&self) -> &str;
    fn genre(&self) -> &str;
    fn language(&self) -> &str;
    fn director(&self) -> &str;
    fn starring(&self) -> &str;
    fn introduction(&self) -> &str;
    fn region(&self) -> &str;
    fn sources(&self) -> &Vec<Vec<String>>;
}