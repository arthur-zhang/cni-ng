pub fn anyhow_io_kind(e: &anyhow::Error) -> Option<std::io::ErrorKind> {
    e.downcast_ref::<std::io::Error>().map(|it| it.kind())
}

pub fn is_already_exists_error(e: &anyhow::Error) -> bool {
    anyhow_io_kind(e).map_or(false, |it| it == std::io::ErrorKind::AlreadyExists)
}

pub fn is_not_found_error(e: &anyhow::Error) -> bool {
    anyhow_io_kind(e).map_or(false, |it| it == std::io::ErrorKind::NotFound)
}
#[macro_export]
macro_rules! wrap_err {
    ($e:expr) => {
        $e.map_err(|e| anyhow::anyhow!("{:?}", e))
    };
}
