//! Almacén cifrado: la base SQLite vive en memoria y en disco solo existe cifrada.
//! Formato del fichero: "CWF1" | m_cost u32 | t_cost u32 | p_cost u32 | salt[16] | nonce[12] | ciphertext(AES-256-GCM)

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use rusqlite::{Connection, MAIN_DB};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

const MAGIC: &[u8; 4] = b"CWF1";
const HEADER_LEN: usize = 4 + 12 + 16 + 12;
const M_COST: u32 = 64 * 1024; // 64 MiB
const T_COST: u32 = 3;
const P_COST: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Contraseña incorrecta o fichero dañado")]
    BadPassword,
    #[error("El fichero no es un almacén de CereWare Finances")]
    BadFormat,
    #[error("La contraseña debe tener al menos 8 caracteres")]
    WeakPassword,
    #[error("Ya existe un almacén")]
    AlreadyExists,
    #[error("No hay ningún almacén creado")]
    NotFound,
    #[error("El almacén está bloqueado")]
    Locked,
    #[error("Error de base de datos: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Error de disco: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error criptográfico")]
    Crypto,
}

impl serde::Serialize for VaultError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub struct Vault {
    pub conn: Connection,
    key: Zeroizing<[u8; 32]>,
    salt: [u8; 16],
    path: PathBuf,
}

fn derive_key(password: &str, salt: &[u8; 16], m: u32, t: u32, p: u32) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let params = Params::new(m, t, p, Some(32)).map_err(|_| VaultError::Crypto)?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|_| VaultError::Crypto)?;
    Ok(key)
}

impl Vault {
    pub fn exists(path: &Path) -> bool {
        path.exists()
    }

    pub fn create(path: &Path, password: &str) -> Result<Self, VaultError> {
        if password.chars().count() < 8 {
            return Err(VaultError::WeakPassword);
        }
        if path.exists() {
            return Err(VaultError::AlreadyExists);
        }
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        let key = derive_key(password, &salt, M_COST, T_COST, P_COST)?;
        let conn = Connection::open_in_memory()?;
        crate::schema::migrate(&conn)?;
        crate::schema::seed(&conn)?;
        let v = Vault { conn, key, salt, path: path.to_path_buf() };
        v.save()?;
        Ok(v)
    }

    pub fn open(path: &Path, password: &str) -> Result<Self, VaultError> {
        if !path.exists() {
            return Err(VaultError::NotFound);
        }
        let data = fs::read(path)?;
        if data.len() < HEADER_LEN || &data[0..4] != MAGIC {
            return Err(VaultError::BadFormat);
        }
        let u = |i: usize| u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
        let (m, t, p) = (u(4), u(8), u(12));
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&data[16..32]);
        let nonce = Nonce::from_slice(&data[32..44]);
        let key = derive_key(password, &salt, m, t, p)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| VaultError::Crypto)?;
        let plain = Zeroizing::new(cipher.decrypt(nonce, &data[HEADER_LEN..]).map_err(|_| VaultError::BadPassword)?);
        let mut conn = Connection::open_in_memory()?;
        conn.deserialize_read_exact(MAIN_DB, &plain[..], plain.len(), false)?;
        crate::schema::migrate(&conn)?;
        Ok(Vault { conn, key, salt, path: path.to_path_buf() })
    }

    /// Serializa la base en memoria, la cifra y la escribe de forma atómica.
    pub fn save(&self) -> Result<(), VaultError> {
        let bytes = self.conn.serialize(MAIN_DB)?;
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let cipher = Aes256Gcm::new_from_slice(self.key.as_ref()).map_err(|_| VaultError::Crypto)?;
        let ct = cipher.encrypt(Nonce::from_slice(&nonce), &bytes[..]).map_err(|_| VaultError::Crypto)?;
        let mut out = Vec::with_capacity(HEADER_LEN + ct.len());
        out.extend_from_slice(MAGIC);
        for v in [M_COST, T_COST, P_COST] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&out)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_wrong_password() {
        let dir = std::env::temp_dir().join(format!("cwf-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("v.cwf");
        {
            let v = Vault::create(&path, "contraseña-segura").unwrap();
            v.conn.execute("INSERT INTO settings(key,value) VALUES('x','42')", []).unwrap();
            v.save().unwrap();
        }
        let raw = fs::read(&path).unwrap();
        assert!(!raw.windows(6).any(|w| w == b"SQLite"), "no debe haber texto plano");
        assert!(matches!(Vault::open(&path, "otra-contraseña"), Err(VaultError::BadPassword)));
        let v = Vault::open(&path, "contraseña-segura").unwrap();
        let x: String = v.conn.query_row("SELECT value FROM settings WHERE key='x'", [], |r| r.get(0)).unwrap();
        assert_eq!(x, "42");
        let n: i64 = v.conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0)).unwrap();
        assert!(n > 10);
        fs::remove_dir_all(dir).ok();
    }
}
