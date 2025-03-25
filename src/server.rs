use std::{fmt, net::SocketAddr};

mod routes;

#[derive(Clone)]
pub struct Headers {
    pub client_addr: SocketAddr,
    pub user_id: String,
    pub client_name: String,
    authorization: String,
}

impl Headers {
    pub fn new(
        client_addr: SocketAddr,
        authorization: &str,
        user_id: &str,
        client_name: &str,
    ) -> Self {
        Self {
            client_addr,
            authorization: authorization.to_string(),
            user_id: user_id.to_string(),
            client_name: client_name.to_string(),
        }
    }

    pub fn verify(self, authorization: &str) -> Option<Self> {
        if self.authorization != authorization {
            return None;
        }
        Some(self)
    }
}

impl fmt::Debug for Headers {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Foo")
            .field("client_addr", &self.client_addr)
            .field("user_id", &self.user_id)
            .field("client_name", &self.client_name)
            .finish()
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
