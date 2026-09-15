//! Metadati di un file GGUF letti senza caricare il modello: si legge solo l'intestazione
//! (chiavi di metadati e descrittori dei tensori), mai i pesi. Su un file da 22 GB significa
//! qualche centinaio di kB.
//!
//! Le chiavi dipendono dall'architettura (`qwen35moe.block_count`, `gpt-oss.block_count`, …):
//! si leggono con il prefisso dichiarato da `general.architecture`. Un campo che il file non
//! ha resta `None`: sconosciuto non è zero.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 4] = b"GGUF";
/// Stringa più lunga di così: file non GGUF o troncato. I template di chat arrivano a ~100 kB.
const MAX_STRING: u64 = 16 << 20;
/// Tensori o chiavi oltre questo numero: intestazione non credibile.
const MAX_COUNT: u64 = 1 << 20;
/// Elementi di un array tenuti in memoria; oltre si conta e si salta (`tokenizer.ggml.tokens`).
const MAX_ARRAY_KEPT: u64 = 64;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value {
    U(u64),
    I(i64),
    F(f64),
    Bool(bool),
    Str(String),
    /// Array corto tenuto per intero.
    Arr(Vec<Value>),
    /// Array lungo: si conserva solo quanti elementi aveva.
    Skipped { elements: u64 },
}

impl Value {
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::U(v) => Some(*v),
            Value::I(v) => u64::try_from(*v).ok(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TensorInfo {
    pub name: String,
    pub dims: Vec<u64>,
    /// Tipo ggml: nome noto (`Q4_K`, `Q8_0`, `MXFP4`) oppure `tipo <n>` se il build è più nuovo di questa tabella.
    pub kind: String,
    pub elements: u64,
}

#[derive(Debug, Clone)]
pub struct Gguf {
    pub version: u32,
    pub tensor_count: u64,
    pub kv: BTreeMap<String, Value>,
    pub tensors: Vec<TensorInfo>,
}

/// Tipi ggml noti a questo build. Un tipo nuovo non si inventa: resta `tipo <n>`.
fn type_name(t: u32) -> String {
    let known = match t {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        6 => "Q5_0",
        7 => "Q5_1",
        8 => "Q8_0",
        9 => "Q8_1",
        10 => "Q2_K",
        11 => "Q3_K",
        12 => "Q4_K",
        13 => "Q5_K",
        14 => "Q6_K",
        15 => "Q8_K",
        16 => "IQ2_XXS",
        17 => "IQ2_XS",
        18 => "IQ3_XXS",
        19 => "IQ1_S",
        20 => "IQ4_NL",
        21 => "IQ3_S",
        22 => "IQ2_S",
        23 => "IQ4_XS",
        24 => "I8",
        25 => "I16",
        26 => "I32",
        27 => "I64",
        28 => "F64",
        29 => "IQ1_M",
        30 => "BF16",
        34 => "TQ1_0",
        35 => "TQ2_0",
        39 => "MXFP4",
        _ => return format!("tipo {t}"),
    };
    known.to_string()
}

struct Reader<R: Read + Seek> {
    r: R,
}

impl<R: Read + Seek> Reader<R> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut b = [0u8; N];
        self.r.read_exact(&mut b).map_err(|e| format!("intestazione GGUF troncata: {e}"))?;
        Ok(b)
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.bytes::<4>()?))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.bytes::<8>()?))
    }

    fn skip(&mut self, n: u64) -> Result<(), String> {
        self.r.seek(SeekFrom::Current(n as i64)).map_err(|e| format!("GGUF: {e}"))?;
        Ok(())
    }

    fn string(&mut self) -> Result<String, String> {
        let len = self.u64()?;
        if len > MAX_STRING {
            return Err(format!("stringa GGUF di {len} byte: file non GGUF o troncato"));
        }
        let mut buf = vec![0u8; len as usize];
        self.r.read_exact(&mut buf).map_err(|e| format!("intestazione GGUF troncata: {e}"))?;
        // I GGUF scrivono UTF-8; un byte storto non deve far fallire tutta la lettura.
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Byte fissi di un tipo scalare, se ne ha: serve a saltare gli array senza leggerli.
    fn scalar_width(t: u32) -> Option<u64> {
        Some(match t {
            0 | 1 | 7 => 1,  // u8, i8, bool
            2 | 3 => 2,      // u16, i16
            4 | 5 | 6 => 4,  // u32, i32, f32
            10 | 11 | 12 => 8, // u64, i64, f64
            _ => return None,
        })
    }

    fn scalar(&mut self, t: u32) -> Result<Value, String> {
        Ok(match t {
            0 => Value::U(self.bytes::<1>()?[0] as u64),
            1 => Value::I(self.bytes::<1>()?[0] as i8 as i64),
            2 => Value::U(u16::from_le_bytes(self.bytes::<2>()?) as u64),
            3 => Value::I(i16::from_le_bytes(self.bytes::<2>()?) as i64),
            4 => Value::U(self.u32()? as u64),
            5 => Value::I(i32::from_le_bytes(self.bytes::<4>()?) as i64),
            6 => Value::F(f32::from_le_bytes(self.bytes::<4>()?) as f64),
            7 => Value::Bool(self.bytes::<1>()?[0] != 0),
            8 => Value::Str(self.string()?),
            10 => Value::U(self.u64()?),
            11 => Value::I(i64::from_le_bytes(self.bytes::<8>()?)),
            12 => Value::F(f64::from_le_bytes(self.bytes::<8>()?)),
            other => return Err(format!("tipo di metadato GGUF sconosciuto: {other}")),
        })
    }

    fn value(&mut self, t: u32) -> Result<Value, String> {
        if t != 9 {
            return self.scalar(t);
        }
        let elem = self.u32()?;
        let count = self.u64()?;
        if count <= MAX_ARRAY_KEPT {
            let mut out = Vec::with_capacity(count as usize);
            for _ in 0..count {
                out.push(self.scalar(elem)?);
            }
            return Ok(Value::Arr(out));
        }
        // Array lungo (i token del tokenizer sono ~150.000): si salta senza allocarlo.
        match Self::scalar_width(elem) {
            Some(w) => self.skip(count.checked_mul(w).ok_or("array GGUF troppo grande")?)?,
            None if elem == 8 => {
                for _ in 0..count {
                    let len = self.u64()?;
                    if len > MAX_STRING {
                        return Err(format!("stringa GGUF di {len} byte in un array"));
                    }
                    self.skip(len)?;
                }
            }
            None => return Err(format!("array GGUF di tipo {elem} non gestito")),
        }
        Ok(Value::Skipped { elements: count })
    }
}

/// Legge l'intestazione: chiavi di metadati e descrittori dei tensori. I pesi non si toccano.
pub fn read(path: &Path) -> Result<Gguf, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    read_from(BufReader::with_capacity(1 << 16, file))
}

pub fn read_from<R: Read + Seek>(inner: R) -> Result<Gguf, String> {
    let mut r = Reader { r: inner };
    if &r.bytes::<4>()? != MAGIC {
        return Err("non è un file GGUF (manca la firma «GGUF» iniziale)".into());
    }
    let version = r.u32()?;
    if !(2..=3).contains(&version) {
        return Err(format!("GGUF versione {version}: questa versione di Aethera legge la 2 e la 3"));
    }
    let tensor_count = r.u64()?;
    let kv_count = r.u64()?;
    if tensor_count > MAX_COUNT || kv_count > MAX_COUNT {
        return Err(format!("intestazione GGUF non credibile: {tensor_count} tensori, {kv_count} chiavi"));
    }

    let mut kv = BTreeMap::new();
    for _ in 0..kv_count {
        let key = r.string()?;
        let t = r.u32()?;
        kv.insert(key, r.value(t)?);
    }

    let mut tensors = Vec::with_capacity(tensor_count.min(4096) as usize);
    for _ in 0..tensor_count {
        let name = r.string()?;
        let n_dims = r.u32()?;
        if n_dims > 8 {
            return Err(format!("tensore «{name}» con {n_dims} dimensioni: intestazione non credibile"));
        }
        let mut dims = Vec::with_capacity(n_dims as usize);
        for _ in 0..n_dims {
            dims.push(r.u64()?);
        }
        let kind = type_name(r.u32()?);
        let _offset = r.u64()?;
        let elements = dims.iter().copied().try_fold(1u64, |a, d| a.checked_mul(d)).unwrap_or(0);
        tensors.push(TensorInfo { name, dims, kind, elements });
    }
    Ok(Gguf { version, tensor_count, kv, tensors })
}

/// Quello che il catalogo mostra di un modello. Ogni campo assente resta sconosciuto.
///
/// Si rilegge anche da `catalog.toml`, dove viene tenuto in cache: `default` fa sì che una chiave
/// scritta da una versione più vecchia (o assente in quel modello) resti semplicemente sconosciuta.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelInfo {
    pub arch: Option<String>,
    pub name: Option<String>,
    pub block_count: Option<u64>,
    pub context_train: Option<u64>,
    pub embedding_length: Option<u64>,
    pub head_count: Option<u64>,
    pub head_count_kv: Option<u64>,
    pub key_length: Option<u64>,
    pub value_length: Option<u64>,
    pub expert_count: Option<u64>,
    pub expert_used_count: Option<u64>,
    /// Layer MTP dichiarati (`<arch>.nextn_predict_layers`).
    pub mtp_layers: Option<u64>,
    /// Sugli ibridi: un blocco ad attenzione piena ogni N, gli altri ricorrenti.
    pub full_attention_interval: Option<u64>,
    pub ssm_conv_kernel: Option<u64>,
    pub ssm_state_size: Option<u64>,
    pub ssm_group_count: Option<u64>,
    pub ssm_inner_size: Option<u64>,
    /// Tipo dominante fra i tensori dei blocchi: la quantizzazione che pesa.
    pub dominant_type: Option<String>,
    /// `general.file_type` grezzo: l'etichetta commerciale (Q4_K_M) la scrive il publisher, non si deduce.
    pub file_type: Option<u64>,
    /// Tipi dei tensori MTP (`blk.N.nextn.*`), per dire se un file MTP separato aggiunge precisione.
    pub mtp_types: Vec<String>,
    /// Vocabolario, dalla lunghezza di `tokenizer.ggml.tokens` (l'array si salta, il conteggio resta).
    pub vocab_size: Option<u64>,
    pub tensor_count: u64,
}

fn key(kv: &BTreeMap<String, Value>, arch: Option<&str>, suffix: &str) -> Option<u64> {
    kv.get(&format!("{}.{suffix}", arch?))?.as_u64()
}

/// Ricava dai metadati i campi che servono al catalogo e alla stima di memoria.
pub fn info(g: &Gguf) -> ModelInfo {
    let arch = g.kv.get("general.architecture").and_then(Value::as_str).map(str::to_string);
    let a = arch.as_deref();
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    let mut mtp_types: Vec<String> = Vec::new();
    for t in &g.tensors {
        if t.name.starts_with("blk.") {
            *counts.entry(t.kind.as_str()).or_default() += t.elements;
        }
        if t.name.contains(".nextn.") && !mtp_types.contains(&t.kind) {
            mtp_types.push(t.kind.clone());
        }
    }
    mtp_types.sort();
    ModelInfo {
        name: g.kv.get("general.name").and_then(Value::as_str).map(str::to_string),
        block_count: key(&g.kv, a, "block_count"),
        context_train: key(&g.kv, a, "context_length"),
        embedding_length: key(&g.kv, a, "embedding_length"),
        head_count: key(&g.kv, a, "attention.head_count"),
        head_count_kv: key(&g.kv, a, "attention.head_count_kv"),
        key_length: key(&g.kv, a, "attention.key_length"),
        value_length: key(&g.kv, a, "attention.value_length"),
        expert_count: key(&g.kv, a, "expert_count"),
        expert_used_count: key(&g.kv, a, "expert_used_count"),
        mtp_layers: key(&g.kv, a, "nextn_predict_layers"),
        full_attention_interval: key(&g.kv, a, "full_attention_interval"),
        ssm_conv_kernel: key(&g.kv, a, "ssm.conv_kernel"),
        ssm_state_size: key(&g.kv, a, "ssm.state_size"),
        ssm_group_count: key(&g.kv, a, "ssm.group_count"),
        ssm_inner_size: key(&g.kv, a, "ssm.inner_size"),
        dominant_type: counts.into_iter().max_by_key(|(_, n)| *n).map(|(k, _)| k.to_string()),
        file_type: g.kv.get("general.file_type").and_then(Value::as_u64),
        mtp_types,
        vocab_size: g.kv.get("tokenizer.ggml.tokens").and_then(|v| match v {
            Value::Skipped { elements } => Some(*elements),
            Value::Arr(a) => Some(a.len() as u64),
            _ => None,
        }),
        tensor_count: g.tensor_count,
        arch,
    }
}

pub fn read_info(path: &Path) -> Result<ModelInfo, String> {
    Ok(info(&read(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Costruisce un GGUF minimo in memoria: la prova non dipende da un file da 22 GB.
    #[derive(Default)]
    struct Build {
        kv: Vec<u8>,
        n_kv: u64,
        tensors: Vec<u8>,
        n_tensors: u64,
    }

    fn gstr(s: &str) -> Vec<u8> {
        let mut v = (s.len() as u64).to_le_bytes().to_vec();
        v.extend_from_slice(s.as_bytes());
        v
    }

    impl Build {
        fn u32(mut self, k: &str, v: u32) -> Self {
            self.kv.extend(gstr(k));
            self.kv.extend(4u32.to_le_bytes());
            self.kv.extend(v.to_le_bytes());
            self.n_kv += 1;
            self
        }

        fn str(mut self, k: &str, v: &str) -> Self {
            self.kv.extend(gstr(k));
            self.kv.extend(8u32.to_le_bytes());
            self.kv.extend(gstr(v));
            self.n_kv += 1;
            self
        }

        /// Array di stringhe lungo, come `tokenizer.ggml.tokens`.
        fn long_strings(mut self, k: &str, n: u64) -> Self {
            self.kv.extend(gstr(k));
            self.kv.extend(9u32.to_le_bytes());
            self.kv.extend(8u32.to_le_bytes());
            self.kv.extend(n.to_le_bytes());
            for i in 0..n {
                self.kv.extend(gstr(&format!("t{i}")));
            }
            self.n_kv += 1;
            self
        }

        fn tensor(mut self, name: &str, dims: &[u64], kind: u32) -> Self {
            self.tensors.extend(gstr(name));
            self.tensors.extend((dims.len() as u32).to_le_bytes());
            for d in dims {
                self.tensors.extend(d.to_le_bytes());
            }
            self.tensors.extend(kind.to_le_bytes());
            self.tensors.extend(0u64.to_le_bytes());
            self.n_tensors += 1;
            self
        }

        fn finish(self) -> Vec<u8> {
            let mut out = MAGIC.to_vec();
            out.extend(3u32.to_le_bytes());
            out.extend(self.n_tensors.to_le_bytes());
            out.extend(self.n_kv.to_le_bytes());
            out.extend(self.kv);
            out.extend(self.tensors);
            out
        }
    }

    fn sample() -> Vec<u8> {
        Build::default()
            .str("general.architecture", "qwen35moe")
            .str("general.name", "Qwen3.6 35B A3B")
            .u32("qwen35moe.block_count", 41)
            .u32("qwen35moe.context_length", 262_144)
            .u32("qwen35moe.attention.head_count_kv", 4)
            .u32("qwen35moe.attention.key_length", 128)
            .u32("qwen35moe.attention.value_length", 128)
            .u32("qwen35moe.expert_count", 256)
            .u32("qwen35moe.expert_used_count", 8)
            .u32("qwen35moe.nextn_predict_layers", 1)
            .u32("qwen35moe.full_attention_interval", 4)
            .long_strings("tokenizer.ggml.tokens", 5000)
            .tensor("blk.0.attn_q.weight", &[4096, 4096], 12)
            .tensor("blk.0.ffn_down.weight", &[4096, 1024], 12)
            .tensor("blk.41.nextn.embed_tokens.weight", &[4096, 128], 8)
            .finish()
    }

    #[test]
    fn reads_header_and_skips_long_arrays() {
        let g = read_from(Cursor::new(sample())).unwrap();
        assert_eq!(g.version, 3);
        assert_eq!(g.tensor_count, 3);
        assert_eq!(g.kv["general.architecture"].as_str(), Some("qwen35moe"));
        // L'array dei token non si alloca: se ne tiene solo la lunghezza.
        assert_eq!(g.kv["tokenizer.ggml.tokens"], Value::Skipped { elements: 5000 });
        assert_eq!(g.tensors[2].kind, "Q8_0");
    }

    #[test]
    fn info_uses_the_architecture_prefix() {
        let i = info(&read_from(Cursor::new(sample())).unwrap());
        assert_eq!(i.arch.as_deref(), Some("qwen35moe"));
        assert_eq!(i.block_count, Some(41));
        assert_eq!(i.context_train, Some(262_144));
        assert_eq!(i.head_count_kv, Some(4));
        assert_eq!(i.mtp_layers, Some(1));
        assert_eq!(i.full_attention_interval, Some(4));
        // I tensori MTP del Qwen3.6 sono già Q8_0: un file MTP separato non aggiunge precisione.
        assert_eq!(i.mtp_types, vec!["Q8_0".to_string()]);
        assert_eq!(i.dominant_type.as_deref(), Some("Q4_K"));
        // Il vocabolario si conta anche se l'array dei token non viene allocato.
        assert_eq!(i.vocab_size, Some(5000));
        // Campi che questo file non dichiara restano sconosciuti, non zero.
        assert_eq!(i.head_count, None);
        assert_eq!(i.ssm_state_size, None);
    }

    #[test]
    fn rejects_files_that_are_not_gguf() {
        let e = read_from(Cursor::new(b"non un gguf qualsiasi".to_vec())).unwrap_err();
        assert!(e.contains("GGUF"), "{e}");
        // Un download a metà: firma giusta, resto troncato.
        let partial = sample()[..40].to_vec();
        assert!(read_from(Cursor::new(partial)).is_err());
    }

    #[test]
    fn refuses_absurd_header_counts() {
        let mut bad = MAGIC.to_vec();
        bad.extend(3u32.to_le_bytes());
        bad.extend(u64::MAX.to_le_bytes());
        bad.extend(1u64.to_le_bytes());
        assert!(read_from(Cursor::new(bad)).unwrap_err().contains("credibile"));
    }
}
