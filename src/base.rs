
pub type Err = anyhow::Error;
pub type Res<T> = Result<T, Err>;
pub type Void = Res<()>;