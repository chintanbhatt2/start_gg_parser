use crate::schema;

#[derive(cynic::Scalar, Debug, Clone, PartialEq)]
#[cynic(graphql_type = "ID")]
pub struct StartggId(IdString);

impl StartggId {
    pub fn as_string(&self) -> &str {
        self.0.as_str()
    }
}

impl From<i64> for StartggId {
    fn from(value: i64) -> Self {
        Self(IdString(value.to_string()))
    }
}

impl From<&str> for StartggId {
    fn from(value: &str) -> Self {
        Self(IdString(value.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq)]
struct IdString(String);

impl IdString {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl serde::Serialize for IdString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for IdString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum IdInput {
            Str(String),
            I64(i64),
            U64(u64),
        }

        match IdInput::deserialize(deserializer)? {
            IdInput::Str(s) => Ok(Self(s)),
            IdInput::I64(n) => Ok(Self(n.to_string())),
            IdInput::U64(n) => Ok(Self(n.to_string())),
        }
    }
}
