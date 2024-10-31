// This file is generated by rust-protobuf 3.3.0. Do not edit
// .proto file is parsed by protoc --rust-out=...
// @generated

// https://github.com/rust-lang/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy::all)]

#![allow(unused_attributes)]
#![cfg_attr(rustfmt, rustfmt::skip)]

#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unused_results)]
#![allow(unused_mut)]

//! Generated file from `signature.proto`

/// Generated files are compatible only with the same version
/// of protobuf runtime.
const _PROTOBUF_VERSION_CHECK: () = ::protobuf::VERSION_3_3_0;

// @@protoc_insertion_point(message:Signature)
#[derive(PartialEq,Clone,Default,Debug)]
pub struct Signature {
    // message fields
    // @@protoc_insertion_point(field:Signature.full_hash)
    pub full_hash: ::std::option::Option<::std::vec::Vec<u8>>,
    // @@protoc_insertion_point(field:Signature.hashes_hash)
    pub hashes_hash: ::std::option::Option<::std::vec::Vec<u8>>,
    // @@protoc_insertion_point(field:Signature.hashes)
    pub hashes: ::std::vec::Vec<signature::Hash>,
    // special fields
    // @@protoc_insertion_point(special_field:Signature.special_fields)
    pub special_fields: ::protobuf::SpecialFields,
}

impl<'a> ::std::default::Default for &'a Signature {
    fn default() -> &'a Signature {
        <Signature as ::protobuf::Message>::default_instance()
    }
}

impl Signature {
    pub fn new() -> Signature {
        ::std::default::Default::default()
    }

    // required bytes full_hash = 1;

    pub fn full_hash(&self) -> &[u8] {
        match self.full_hash.as_ref() {
            Some(v) => v,
            None => &[],
        }
    }

    pub fn clear_full_hash(&mut self) {
        self.full_hash = ::std::option::Option::None;
    }

    pub fn has_full_hash(&self) -> bool {
        self.full_hash.is_some()
    }

    // Param is passed by value, moved
    pub fn set_full_hash(&mut self, v: ::std::vec::Vec<u8>) {
        self.full_hash = ::std::option::Option::Some(v);
    }

    // Mutable pointer to the field.
    // If field is not initialized, it is initialized with default value first.
    pub fn mut_full_hash(&mut self) -> &mut ::std::vec::Vec<u8> {
        if self.full_hash.is_none() {
            self.full_hash = ::std::option::Option::Some(::std::vec::Vec::new());
        }
        self.full_hash.as_mut().unwrap()
    }

    // Take field
    pub fn take_full_hash(&mut self) -> ::std::vec::Vec<u8> {
        self.full_hash.take().unwrap_or_else(|| ::std::vec::Vec::new())
    }

    // required bytes hashes_hash = 2;

    pub fn hashes_hash(&self) -> &[u8] {
        match self.hashes_hash.as_ref() {
            Some(v) => v,
            None => &[],
        }
    }

    pub fn clear_hashes_hash(&mut self) {
        self.hashes_hash = ::std::option::Option::None;
    }

    pub fn has_hashes_hash(&self) -> bool {
        self.hashes_hash.is_some()
    }

    // Param is passed by value, moved
    pub fn set_hashes_hash(&mut self, v: ::std::vec::Vec<u8>) {
        self.hashes_hash = ::std::option::Option::Some(v);
    }

    // Mutable pointer to the field.
    // If field is not initialized, it is initialized with default value first.
    pub fn mut_hashes_hash(&mut self) -> &mut ::std::vec::Vec<u8> {
        if self.hashes_hash.is_none() {
            self.hashes_hash = ::std::option::Option::Some(::std::vec::Vec::new());
        }
        self.hashes_hash.as_mut().unwrap()
    }

    // Take field
    pub fn take_hashes_hash(&mut self) -> ::std::vec::Vec<u8> {
        self.hashes_hash.take().unwrap_or_else(|| ::std::vec::Vec::new())
    }

    fn generated_message_descriptor_data() -> ::protobuf::reflect::GeneratedMessageDescriptorData {
        let mut fields = ::std::vec::Vec::with_capacity(3);
        let mut oneofs = ::std::vec::Vec::with_capacity(0);
        fields.push(::protobuf::reflect::rt::v2::make_option_accessor::<_, _>(
            "full_hash",
            |m: &Signature| { &m.full_hash },
            |m: &mut Signature| { &mut m.full_hash },
        ));
        fields.push(::protobuf::reflect::rt::v2::make_option_accessor::<_, _>(
            "hashes_hash",
            |m: &Signature| { &m.hashes_hash },
            |m: &mut Signature| { &mut m.hashes_hash },
        ));
        fields.push(::protobuf::reflect::rt::v2::make_vec_simpler_accessor::<_, _>(
            "hashes",
            |m: &Signature| { &m.hashes },
            |m: &mut Signature| { &mut m.hashes },
        ));
        ::protobuf::reflect::GeneratedMessageDescriptorData::new_2::<Signature>(
            "Signature",
            fields,
            oneofs,
        )
    }
}

impl ::protobuf::Message for Signature {
    const NAME: &'static str = "Signature";

    fn is_initialized(&self) -> bool {
        if self.full_hash.is_none() {
            return false;
        }
        if self.hashes_hash.is_none() {
            return false;
        }
        for v in &self.hashes {
            if !v.is_initialized() {
                return false;
            }
        };
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::Result<()> {
        while let Some(tag) = is.read_raw_tag_or_eof()? {
            match tag {
                10 => {
                    self.full_hash = ::std::option::Option::Some(is.read_bytes()?);
                },
                18 => {
                    self.hashes_hash = ::std::option::Option::Some(is.read_bytes()?);
                },
                26 => {
                    self.hashes.push(is.read_message()?);
                },
                tag => {
                    ::protobuf::rt::read_unknown_or_skip_group(tag, is, self.special_fields.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u64 {
        let mut my_size = 0;
        if let Some(v) = self.full_hash.as_ref() {
            my_size += ::protobuf::rt::bytes_size(1, &v);
        }
        if let Some(v) = self.hashes_hash.as_ref() {
            my_size += ::protobuf::rt::bytes_size(2, &v);
        }
        for value in &self.hashes {
            let len = value.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint64_size(len) + len;
        };
        my_size += ::protobuf::rt::unknown_fields_size(self.special_fields.unknown_fields());
        self.special_fields.cached_size().set(my_size as u32);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::Result<()> {
        if let Some(v) = self.full_hash.as_ref() {
            os.write_bytes(1, v)?;
        }
        if let Some(v) = self.hashes_hash.as_ref() {
            os.write_bytes(2, v)?;
        }
        for v in &self.hashes {
            ::protobuf::rt::write_message_field_with_cached_size(3, v, os)?;
        };
        os.write_unknown_fields(self.special_fields.unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn special_fields(&self) -> &::protobuf::SpecialFields {
        &self.special_fields
    }

    fn mut_special_fields(&mut self) -> &mut ::protobuf::SpecialFields {
        &mut self.special_fields
    }

    fn new() -> Signature {
        Signature::new()
    }

    fn clear(&mut self) {
        self.full_hash = ::std::option::Option::None;
        self.hashes_hash = ::std::option::Option::None;
        self.hashes.clear();
        self.special_fields.clear();
    }

    fn default_instance() -> &'static Signature {
        static instance: Signature = Signature {
            full_hash: ::std::option::Option::None,
            hashes_hash: ::std::option::Option::None,
            hashes: ::std::vec::Vec::new(),
            special_fields: ::protobuf::SpecialFields::new(),
        };
        &instance
    }
}

impl ::protobuf::MessageFull for Signature {
    fn descriptor() -> ::protobuf::reflect::MessageDescriptor {
        static descriptor: ::protobuf::rt::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::rt::Lazy::new();
        descriptor.get(|| file_descriptor().message_by_package_relative_name("Signature").unwrap()).clone()
    }
}

impl ::std::fmt::Display for Signature {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for Signature {
    type RuntimeType = ::protobuf::reflect::rt::RuntimeTypeMessage<Self>;
}

/// Nested message and enums of message `Signature`
pub mod signature {
    // @@protoc_insertion_point(message:Signature.Hash)
    #[derive(PartialEq,Clone,Default,Debug)]
    pub struct Hash {
        // message fields
        // @@protoc_insertion_point(field:Signature.Hash.hash)
        pub hash: ::std::option::Option<::std::vec::Vec<u8>>,
        // @@protoc_insertion_point(field:Signature.Hash.offset)
        pub offset: ::std::option::Option<i64>,
        // special fields
        // @@protoc_insertion_point(special_field:Signature.Hash.special_fields)
        pub special_fields: ::protobuf::SpecialFields,
    }

    impl<'a> ::std::default::Default for &'a Hash {
        fn default() -> &'a Hash {
            <Hash as ::protobuf::Message>::default_instance()
        }
    }

    impl Hash {
        pub fn new() -> Hash {
            ::std::default::Default::default()
        }

        // required bytes hash = 1;

        pub fn hash(&self) -> &[u8] {
            match self.hash.as_ref() {
                Some(v) => v,
                None => &[],
            }
        }

        pub fn clear_hash(&mut self) {
            self.hash = ::std::option::Option::None;
        }

        pub fn has_hash(&self) -> bool {
            self.hash.is_some()
        }

        // Param is passed by value, moved
        pub fn set_hash(&mut self, v: ::std::vec::Vec<u8>) {
            self.hash = ::std::option::Option::Some(v);
        }

        // Mutable pointer to the field.
        // If field is not initialized, it is initialized with default value first.
        pub fn mut_hash(&mut self) -> &mut ::std::vec::Vec<u8> {
            if self.hash.is_none() {
                self.hash = ::std::option::Option::Some(::std::vec::Vec::new());
            }
            self.hash.as_mut().unwrap()
        }

        // Take field
        pub fn take_hash(&mut self) -> ::std::vec::Vec<u8> {
            self.hash.take().unwrap_or_else(|| ::std::vec::Vec::new())
        }

        // required int64 offset = 2;

        pub fn offset(&self) -> i64 {
            self.offset.unwrap_or(0)
        }

        pub fn clear_offset(&mut self) {
            self.offset = ::std::option::Option::None;
        }

        pub fn has_offset(&self) -> bool {
            self.offset.is_some()
        }

        // Param is passed by value, moved
        pub fn set_offset(&mut self, v: i64) {
            self.offset = ::std::option::Option::Some(v);
        }

        pub(in super) fn generated_message_descriptor_data() -> ::protobuf::reflect::GeneratedMessageDescriptorData {
            let mut fields = ::std::vec::Vec::with_capacity(2);
            let mut oneofs = ::std::vec::Vec::with_capacity(0);
            fields.push(::protobuf::reflect::rt::v2::make_option_accessor::<_, _>(
                "hash",
                |m: &Hash| { &m.hash },
                |m: &mut Hash| { &mut m.hash },
            ));
            fields.push(::protobuf::reflect::rt::v2::make_option_accessor::<_, _>(
                "offset",
                |m: &Hash| { &m.offset },
                |m: &mut Hash| { &mut m.offset },
            ));
            ::protobuf::reflect::GeneratedMessageDescriptorData::new_2::<Hash>(
                "Signature.Hash",
                fields,
                oneofs,
            )
        }
    }

    impl ::protobuf::Message for Hash {
        const NAME: &'static str = "Hash";

        fn is_initialized(&self) -> bool {
            if self.hash.is_none() {
                return false;
            }
            if self.offset.is_none() {
                return false;
            }
            true
        }

        fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::Result<()> {
            while let Some(tag) = is.read_raw_tag_or_eof()? {
                match tag {
                    10 => {
                        self.hash = ::std::option::Option::Some(is.read_bytes()?);
                    },
                    16 => {
                        self.offset = ::std::option::Option::Some(is.read_int64()?);
                    },
                    tag => {
                        ::protobuf::rt::read_unknown_or_skip_group(tag, is, self.special_fields.mut_unknown_fields())?;
                    },
                };
            }
            ::std::result::Result::Ok(())
        }

        // Compute sizes of nested messages
        #[allow(unused_variables)]
        fn compute_size(&self) -> u64 {
            let mut my_size = 0;
            if let Some(v) = self.hash.as_ref() {
                my_size += ::protobuf::rt::bytes_size(1, &v);
            }
            if let Some(v) = self.offset {
                my_size += ::protobuf::rt::int64_size(2, v);
            }
            my_size += ::protobuf::rt::unknown_fields_size(self.special_fields.unknown_fields());
            self.special_fields.cached_size().set(my_size as u32);
            my_size
        }

        fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::Result<()> {
            if let Some(v) = self.hash.as_ref() {
                os.write_bytes(1, v)?;
            }
            if let Some(v) = self.offset {
                os.write_int64(2, v)?;
            }
            os.write_unknown_fields(self.special_fields.unknown_fields())?;
            ::std::result::Result::Ok(())
        }

        fn special_fields(&self) -> &::protobuf::SpecialFields {
            &self.special_fields
        }

        fn mut_special_fields(&mut self) -> &mut ::protobuf::SpecialFields {
            &mut self.special_fields
        }

        fn new() -> Hash {
            Hash::new()
        }

        fn clear(&mut self) {
            self.hash = ::std::option::Option::None;
            self.offset = ::std::option::Option::None;
            self.special_fields.clear();
        }

        fn default_instance() -> &'static Hash {
            static instance: Hash = Hash {
                hash: ::std::option::Option::None,
                offset: ::std::option::Option::None,
                special_fields: ::protobuf::SpecialFields::new(),
            };
            &instance
        }
    }

    impl ::protobuf::MessageFull for Hash {
        fn descriptor() -> ::protobuf::reflect::MessageDescriptor {
            static descriptor: ::protobuf::rt::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::rt::Lazy::new();
            descriptor.get(|| super::file_descriptor().message_by_package_relative_name("Signature.Hash").unwrap()).clone()
        }
    }

    impl ::std::fmt::Display for Hash {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            ::protobuf::text_format::fmt(self, f)
        }
    }

    impl ::protobuf::reflect::ProtobufValue for Hash {
        type RuntimeType = ::protobuf::reflect::rt::RuntimeTypeMessage<Self>;
    }
}

static file_descriptor_proto_data: &'static [u8] = b"\
    \n\x0fsignature.proto\"\xa6\x01\n\tSignature\x12\x1b\n\tfull_hash\x18\
    \x01\x20\x02(\x0cR\x08fullHash\x12\x1f\n\x0bhashes_hash\x18\x02\x20\x02(\
    \x0cR\nhashesHash\x12'\n\x06hashes\x18\x03\x20\x03(\x0b2\x0f.Signature.H\
    ashR\x06hashes\x1a2\n\x04Hash\x12\x12\n\x04hash\x18\x01\x20\x02(\x0cR\
    \x04hash\x12\x16\n\x06offset\x18\x02\x20\x02(\x03R\x06offsetJ\xb3\x03\n\
    \x06\x12\x04\0\0\t\x01\n\x08\n\x01\x0c\x12\x03\0\0\x12\n\n\n\x02\x04\0\
    \x12\x04\x01\0\t\x01\n\n\n\x03\x04\0\x01\x12\x03\x01\x08\x11\n\x0b\n\x04\
    \x04\0\x02\0\x12\x03\x02\x08%\n\x0c\n\x05\x04\0\x02\0\x04\x12\x03\x02\
    \x08\x10\n\x0c\n\x05\x04\0\x02\0\x05\x12\x03\x02\x11\x16\n\x0c\n\x05\x04\
    \0\x02\0\x01\x12\x03\x02\x17\x20\n\x0c\n\x05\x04\0\x02\0\x03\x12\x03\x02\
    #$\n\x0b\n\x04\x04\0\x02\x01\x12\x03\x03\x08'\n\x0c\n\x05\x04\0\x02\x01\
    \x04\x12\x03\x03\x08\x10\n\x0c\n\x05\x04\0\x02\x01\x05\x12\x03\x03\x11\
    \x16\n\x0c\n\x05\x04\0\x02\x01\x01\x12\x03\x03\x17\"\n\x0c\n\x05\x04\0\
    \x02\x01\x03\x12\x03\x03%&\n\x0c\n\x04\x04\0\x03\0\x12\x04\x04\x08\x07\t\
    \n\x0c\n\x05\x04\0\x03\0\x01\x12\x03\x04\x10\x14\n\r\n\x06\x04\0\x03\0\
    \x02\0\x12\x03\x05\x10(\n\x0e\n\x07\x04\0\x03\0\x02\0\x04\x12\x03\x05\
    \x10\x18\n\x0e\n\x07\x04\0\x03\0\x02\0\x05\x12\x03\x05\x19\x1e\n\x0e\n\
    \x07\x04\0\x03\0\x02\0\x01\x12\x03\x05\x1f#\n\x0e\n\x07\x04\0\x03\0\x02\
    \0\x03\x12\x03\x05&'\n\r\n\x06\x04\0\x03\0\x02\x01\x12\x03\x06\x10*\n\
    \x0e\n\x07\x04\0\x03\0\x02\x01\x04\x12\x03\x06\x10\x18\n\x0e\n\x07\x04\0\
    \x03\0\x02\x01\x05\x12\x03\x06\x19\x1e\n\x0e\n\x07\x04\0\x03\0\x02\x01\
    \x01\x12\x03\x06\x1f%\n\x0e\n\x07\x04\0\x03\0\x02\x01\x03\x12\x03\x06()\
    \n\x0b\n\x04\x04\0\x02\x02\x12\x03\x08\x08!\n\x0c\n\x05\x04\0\x02\x02\
    \x04\x12\x03\x08\x08\x10\n\x0c\n\x05\x04\0\x02\x02\x06\x12\x03\x08\x11\
    \x15\n\x0c\n\x05\x04\0\x02\x02\x01\x12\x03\x08\x16\x1c\n\x0c\n\x05\x04\0\
    \x02\x02\x03\x12\x03\x08\x1f\x20\
";

/// `FileDescriptorProto` object which was a source for this generated file
fn file_descriptor_proto() -> &'static ::protobuf::descriptor::FileDescriptorProto {
    static file_descriptor_proto_lazy: ::protobuf::rt::Lazy<::protobuf::descriptor::FileDescriptorProto> = ::protobuf::rt::Lazy::new();
    file_descriptor_proto_lazy.get(|| {
        ::protobuf::Message::parse_from_bytes(file_descriptor_proto_data).unwrap()
    })
}

/// `FileDescriptor` object which allows dynamic access to files
pub fn file_descriptor() -> &'static ::protobuf::reflect::FileDescriptor {
    static generated_file_descriptor_lazy: ::protobuf::rt::Lazy<::protobuf::reflect::GeneratedFileDescriptor> = ::protobuf::rt::Lazy::new();
    static file_descriptor: ::protobuf::rt::Lazy<::protobuf::reflect::FileDescriptor> = ::protobuf::rt::Lazy::new();
    file_descriptor.get(|| {
        let generated_file_descriptor = generated_file_descriptor_lazy.get(|| {
            let mut deps = ::std::vec::Vec::with_capacity(0);
            let mut messages = ::std::vec::Vec::with_capacity(2);
            messages.push(Signature::generated_message_descriptor_data());
            messages.push(signature::Hash::generated_message_descriptor_data());
            let mut enums = ::std::vec::Vec::with_capacity(0);
            ::protobuf::reflect::GeneratedFileDescriptor::new_generated(
                file_descriptor_proto(),
                deps,
                messages,
                enums,
            )
        });
        ::protobuf::reflect::FileDescriptor::new_generated_2(generated_file_descriptor)
    })
}
