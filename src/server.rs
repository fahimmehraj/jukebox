use std::net::SocketAddr;

mod routes;

#[derive(Debug, Clone)]
pub struct Headers {
    pub client_addr: SocketAddr,
    pub user_id: String,
    pub client_name: String,
}

impl Headers {
    pub fn new(client_addr: SocketAddr, user_id: &str, client_name: &str) -> Self {
        Self {
            client_addr,
            user_id: user_id.to_string(),
            client_name: client_name.to_string(),
        }
    }
}

/// More fields coming soon
pub struct Server {
    password: String,
    address: SocketAddr,
}

impl Server {
    pub fn _new(password: String, address: SocketAddr) -> Self {
        Self { password, address }
    }

    pub async fn run(self) -> Result<(), std::io::Error> {
        let app = routes::app(self.password);
        let listener = tokio::net::TcpListener::bind(self.address).await?;
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
        Ok(())
    }
}
