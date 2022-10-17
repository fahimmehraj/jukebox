pub struct Client {
    authorization: String,
    user_id: String,
    client_name: String,
}

impl Client {
    pub fn new(authorization: String, user_id: String, client_name: String) -> Self {
        Self {
            authorization,
            user_id,
            client_name,
        }
    }
}