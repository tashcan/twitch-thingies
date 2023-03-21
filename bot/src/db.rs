pub(crate) struct TashbotDb {
    pool: mysql::Pool,
}

impl TashbotDb {
    pub fn new(pool: mysql::Pool) -> Self {
        Self { pool }
    }
}
