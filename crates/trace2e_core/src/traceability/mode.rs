#[derive(Default, Debug, Clone, Copy, clap::ValueEnum, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Pull,
    Push,
}
