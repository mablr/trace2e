#[derive(Default, Debug, Clone, clap::ValueEnum, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Pull,
    Push,
}
