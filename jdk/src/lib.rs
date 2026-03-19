pub const JAVA_LANG_CLASS_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/java/lang/Class.class"));

pub const JAVA_LANG_OBJECT_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/java/lang/Object.class"));

pub const JAVA_LANG_STRING_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/java/lang/String.class"));

pub const JAVA_LANG_THREAD_GROUP_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/java/lang/ThreadGroup.class"));

pub const JAVA_LANG_THREAD_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/java/lang/Thread.class"));

pub const JAVA_LANG_SYSTEM_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/java/lang/System.class"));

pub const JAVA_IO_PRINT_STREAM_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/java/io/PrintStream.class"));
