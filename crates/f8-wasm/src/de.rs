//! Strict `JsValue` Serde deserializer that enforces `#[serde(deny_unknown_fields)]`.
//!
//! `serde-wasm-bindgen`'s `Deserializer::deserialize_struct` looks up only declared struct fields
//! by name and ignores extra own properties on JS objects. Routing struct deserialization through
//! `Object.entries` ensures every enumerable property is visited across nested structs, maps,
//! sequences, and enums.

use js_sys::{Array, ArrayBuffer, Map, Object, Reflect, Symbol, Uint8Array};
use serde::de::value::{MapDeserializer, SeqDeserializer};
use serde::de::{self, DeserializeOwned, Deserializer as _, IntoDeserializer};
use serde_wasm_bindgen::{Deserializer as BaseDeserializer, Error};
use wasm_bindgen::{JsCast, JsValue};

type Result<T> = std::result::Result<T, Error>;

/// Deserializes a [`JsValue`] into `T` while enforcing `#[serde(deny_unknown_fields)]`.
pub fn from_value<T: DeserializeOwned>(value: JsValue) -> Result<T> {
    T::deserialize(StrictDeserializer::from(value))
}

struct StrictSeqAccess {
    iter: js_sys::IntoIter,
}

impl<'de> de::SeqAccess<'de> for StrictSeqAccess {
    type Error = Error;

    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        match self.iter.next().transpose()? {
            Some(value) => seed.deserialize(StrictDeserializer::from(value)).map(Some),
            None => Ok(None),
        }
    }
}

struct StrictMapAccess {
    iter: js_sys::IntoIter,
    next_value: Option<StrictDeserializer>,
}

impl StrictMapAccess {
    const fn new(iter: js_sys::IntoIter) -> Self {
        Self {
            iter,
            next_value: None,
        }
    }
}

impl<'de> de::MapAccess<'de> for StrictMapAccess {
    type Error = Error;

    fn next_key_seed<K: de::DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        match self.iter.next().transpose()? {
            Some(pair) => {
                let (key, value) = try_convert_pair(pair)?;
                self.next_value = Some(value);
                seed.deserialize(key).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        match self.next_value.take() {
            Some(value) => seed.deserialize(value),
            None => Err(de::Error::custom("map value deserialized before key")),
        }
    }
}

struct StrictEnumAccess {
    tag: StrictDeserializer,
    payload: StrictDeserializer,
}

impl<'de> de::EnumAccess<'de> for StrictEnumAccess {
    type Error = Error;
    type Variant = StrictDeserializer;

    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant)> {
        Ok((seed.deserialize(self.tag)?, self.payload))
    }
}

fn convert_pair(pair: JsValue) -> (StrictDeserializer, StrictDeserializer) {
    let pair = pair.unchecked_into::<Array>();
    (pair.get(0).into(), pair.get(1).into())
}

fn try_convert_pair(pair: JsValue) -> Result<(StrictDeserializer, StrictDeserializer)> {
    let pair = pair
        .dyn_into::<Array>()
        .map_err(|_| Error::new("expected [key, value] entry array"))?;
    if pair.length() != 2 {
        return Err(Error::new("expected [key, value] entry array of length 2"));
    }
    Ok((pair.get(0).into(), pair.get(1).into()))
}

fn has_symbol_iterator(value: &JsValue) -> bool {
    Reflect::has(value, &Symbol::iterator()).unwrap_or(false)
}

struct StrictDeserializer {
    value: JsValue,
}

impl From<JsValue> for StrictDeserializer {
    fn from(value: JsValue) -> Self {
        Self { value }
    }
}

impl<'de> IntoDeserializer<'de, Error> for StrictDeserializer {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl StrictDeserializer {
    fn is_nullish(&self) -> bool {
        self.value.is_null() || self.value.is_undefined()
    }

    fn is_bytes(&self) -> bool {
        self.value.is_instance_of::<Uint8Array>() || self.value.is_instance_of::<ArrayBuffer>()
    }

    fn is_plain_object(&self) -> bool {
        self.value.is_object()
            && !Array::is_array(&self.value)
            && !self.is_bytes()
            && !has_symbol_iterator(&self.value)
    }

    fn is_any_map_object(&self) -> bool {
        self.value.is_object()
            && !Array::is_array(&self.value)
            && !self.is_bytes()
            && (!has_symbol_iterator(&self.value) || self.value.has_type::<Map>())
    }

    fn into_base(self) -> BaseDeserializer {
        BaseDeserializer::from(self.value)
    }

    fn deserialize_from_array<'de, V: de::Visitor<'de>>(
        visitor: V,
        array: &Array,
    ) -> Result<V::Value> {
        visitor.visit_seq(SeqDeserializer::new(
            array.iter().map(StrictDeserializer::from),
        ))
    }

    fn invalid_type<'de, V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if Array::is_array(&self.value) {
            return Err(de::Error::invalid_type(de::Unexpected::Seq, &visitor));
        }
        if self.value.is_object() {
            let desc = format!("{:?}", self.value);
            return Err(de::Error::invalid_type(
                de::Unexpected::Other(&desc),
                &visitor,
            ));
        }
        self.into_base().deserialize_struct("", &[], visitor)
    }
}

impl<'de> de::Deserializer<'de> for StrictDeserializer {
    type Error = Error;

    fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if Array::is_array(&self.value) {
            self.deserialize_seq(visitor)
        } else if self.is_any_map_object() {
            self.deserialize_map(visitor)
        } else {
            self.into_base().deserialize_any(visitor)
        }
    }

    fn deserialize_bool<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_bool(visitor)
    }

    fn deserialize_i8<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_i8(visitor)
    }

    fn deserialize_i16<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_i16(visitor)
    }

    fn deserialize_i32<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_i32(visitor)
    }

    fn deserialize_i64<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_i64(visitor)
    }

    fn deserialize_i128<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_i128(visitor)
    }

    fn deserialize_u8<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_u8(visitor)
    }

    fn deserialize_u16<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_u16(visitor)
    }

    fn deserialize_u32<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_u32(visitor)
    }

    fn deserialize_u64<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_u64(visitor)
    }

    fn deserialize_u128<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_u128(visitor)
    }

    fn deserialize_f32<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_f32(visitor)
    }

    fn deserialize_f64<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_f64(visitor)
    }

    fn deserialize_char<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_char(visitor)
    }

    fn deserialize_str<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_str(visitor)
    }

    fn deserialize_string<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_string(visitor)
    }

    fn deserialize_bytes<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_bytes(visitor)
    }

    fn deserialize_byte_buf<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_byte_buf(visitor)
    }

    fn deserialize_option<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if self.is_nullish() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_unit(visitor)
    }

    fn deserialize_unit_struct<V: de::Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        self.into_base().deserialize_unit_struct(name, visitor)
    }

    fn deserialize_newtype_struct<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if let Some(arr) = self.value.dyn_ref::<Array>() {
            Self::deserialize_from_array(visitor, arr)
        } else if self.value.is_object()
            && let Some(iter) = js_sys::try_iter(&self.value)?
        {
            visitor.visit_seq(StrictSeqAccess { iter })
        } else {
            self.invalid_type(visitor)
        }
    }

    fn deserialize_tuple<V: de::Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if !self.value.is_object() {
            return self.invalid_type(visitor);
        }
        match js_sys::try_iter(&self.value)? {
            Some(iter) => visitor.visit_map(StrictMapAccess::new(iter)),
            None => {
                let entries = Object::entries(self.value.unchecked_ref());
                visitor.visit_map(MapDeserializer::new(entries.iter().map(convert_pair)))
            }
        }
    }

    fn deserialize_struct<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        if self.is_plain_object() {
            let entries = Object::entries(self.value.unchecked_ref());
            visitor.visit_map(MapDeserializer::new(entries.iter().map(convert_pair)))
        } else {
            self.invalid_type(visitor)
        }
    }

    fn deserialize_enum<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let access = if self.value.is_string() {
            StrictEnumAccess {
                tag: self,
                payload: Self::from(JsValue::UNDEFINED),
            }
        } else if self.is_plain_object() {
            let entries = Object::entries(self.value.unchecked_ref());
            if entries.length() != 1 {
                let len = usize::try_from(entries.length()).unwrap_or(usize::MAX);
                return Err(de::Error::invalid_length(len, &"1"));
            }
            let (tag, payload) = convert_pair(entries.get(0));
            StrictEnumAccess { tag, payload }
        } else {
            return self.invalid_type(visitor);
        };
        visitor.visit_enum(access)
    }

    fn deserialize_identifier<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_identifier(visitor)
    }

    fn deserialize_ignored_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.into_base().deserialize_ignored_any(visitor)
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

impl<'de> de::VariantAccess<'de> for StrictDeserializer {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        de::Deserialize::deserialize(self)
    }

    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        seed.deserialize(self)
    }

    fn tuple_variant<V: de::Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        de::Deserializer::deserialize_tuple(self, len, visitor)
    }

    fn struct_variant<V: de::Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        de::Deserializer::deserialize_struct(self, "", fields, visitor)
    }
}
