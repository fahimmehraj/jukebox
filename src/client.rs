pub mod player;

use std::collections::HashMap;

use player::Player;

pub struct Headers {
    authorization: String,
    user_id: String,
    client_name: String,
}

impl Headers {
    pub fn new(authorization: String, user_id: String, client_name: String) -> Self {
        Self {
            authorization,
            user_id,
            client_name,
        }
    }

    fn verify(self, authorization: &str) -> Option<Self> {
        if self.authorization != authorization {
            return None
        }
        Some(self)
    }

    pub fn build(self, authorization: &str) -> Option<Client> {
        match self.verify(authorization) {
            Some(headers) => Some(Client {
                user_id: headers.user_id,
                client_name: headers.client_name,
                players: HashMap::new(),
            }),
            None => None
        }
    }
}

pub struct Client {
    user_id: String,
    client_name: String,
    players: HashMap<String, Player>,
}

impl Client {
    pub fn id(&self) -> String {
        self.user_id.clone()
    }

    pub fn client_name(&self) -> String {
        self.client_name.clone()
    }

    pub fn add_player(&mut self, player: Player) {
        self.players.insert(player.guild_id(), player);
    }
}