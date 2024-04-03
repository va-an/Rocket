use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::str::FromStr;

use arc_swap::ArcSwap;
use zerocopy::{IntoBytes, NoCell, TryFromBytes};
use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD as ENCODING};

use crate::key::Rotatable;
use crate::{Session, SessionId};

#[derive(Clone, Debug)]
pub struct Tokenizer {
    pub(super) state: Arc<ArcSwap<TokenizerState>>,
}

#[derive(Debug)]
pub struct TokenizerState {
    pub(super) age: AtomicU32,
    pub(super) key: Rotatable<[u8; 32]>,
}

#[derive(Debug, Clone)]
pub struct Token {
    // The plaintext token data.
    data: TokenData,
    // This is a keyed hash of the above.
    hash: blake3::Hash,
}

#[derive(Debug, Copy, Clone, IntoBytes, NoCell, TryFromBytes)]
#[repr(packed)]
struct TokenData {
    // The `age` and `generation` are a logical timestamp.
    age: u32,
    generation: u32,
    // Session-specifc data.
    session: u64,
    // The context this token should be use in.
    context: Context,
    nonce: [u8; 7],
}

#[derive(Debug, Copy, Clone, IntoBytes, NoCell, TryFromBytes)]
#[repr(u8)]
enum Context {
    Javascript,
    Form,
}

impl TokenizerState {
    pub fn new(key: Rotatable<[u8; 32]>) -> Self {
        Self { age: AtomicU32::new(0), key }
    }
}

impl Tokenizer {
    pub fn new() -> Self {
        let key = Rotatable::generate();
        Self { state: Arc::new(ArcSwap::new(Arc::new(TokenizerState::new(key)))) }
    }

    pub fn rotate(&self) {
        let mut new_state = TokenizerState::new(self.state.load().key.clone());
        new_state.key.generate_and_rotate().expect("key generation");
        self.state.store(Arc::new(new_state));
    }

    fn token(&self, context: Context, session_id: SessionId) -> Token {
        let key = &self.state.load().key;
        let age = self.state.load().age.fetch_add(1, Ordering::AcqRel);
        Token::new(key, age, key.generation(), session_id.value(), context)
    }

    pub fn js_token(&self, session: SessionId) -> Token {
        self.token(Context::Javascript, session)
    }

    pub fn form_token(&self, session: SessionId) -> Token {
        self.token(Context::Form, session)
    }

    pub fn validate(&self, token: &Token, session: &Session) -> bool {
        let state = self.state.load();
        if state.key.generation().saturating_sub(token.data.generation) <= 1 {
            let valid_session = session.iter().any(|id| token.data.session == id.value());
            let authentic = state.key.iter().any(|key| token.is_authentic(key));
            return valid_session && authentic;
        }

        false
    }
}

impl Token {
    fn new<K>(key: K, age: u32, generation: u32, session: u64, context: Context) -> Self
        where K: AsRef<[u8; 32]>
    {
        let data = TokenData { age, generation, nonce: rand::random(), session, context };
        Token { hash: data.hash(key.as_ref()), data }
    }

    fn is_authentic(&self, key: &[u8; 32]) -> bool {
        self.data.hash(key) == self.hash
    }
}

impl TokenData {
    fn hash(&self, key: &[u8; 32]) -> blake3::Hash {
        blake3::keyed_hash(key, self.as_bytes())
    }
}

const ENCODED_DATA_LEN: usize = crate::base64_len::<TokenData>();
const ENCODED_HASH_LEN: usize = crate::base64_len::<blake3::Hash>();

impl ToString for Token {
    fn to_string(&self) -> String {
        let mut string = String::with_capacity(ENCODED_DATA_LEN + ENCODED_HASH_LEN);
        ENCODING.encode_string(self.data.as_bytes(), &mut string);
        ENCODING.encode_string(self.hash.as_bytes(), &mut string);
        string
    }
}

impl FromStr for Token {
    type Err = ();

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        if string.len() != ENCODED_DATA_LEN + ENCODED_HASH_LEN {
            return Err(());
        }

        let (data_str, hash_str) = string.split_at(ENCODED_DATA_LEN);
        let data_bytes = ENCODING.decode(data_str).map_err(|_| ())?;
        let hash_bytes = ENCODING.decode(hash_str).map_err(|_| ())?;
        let data = TokenData::try_read_from(&data_bytes).ok_or(())?;
        let hash = blake3::Hash::from_bytes(hash_bytes.try_into().map_err(|_| ())?);
        Ok(Token { data, hash })
    }
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
