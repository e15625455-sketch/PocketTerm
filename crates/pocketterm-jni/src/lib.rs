//! JNI bridge for the Android terminal UI.

use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jint, jobjectArray};
use pocketterm_core::{Terminal, cancel_running_process};
use std::path::{Path, PathBuf};

#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(
    _vm: *mut jni::sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jint {
    jni::sys::JNI_VERSION_1_6
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_pocketterm_android_MainActivity_executeCommand(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    files_dir: JString<'_>,
    current_dir: JString<'_>,
    command: JString<'_>,
) -> jobjectArray {
    let Ok(files_dir) = env.get_string(&files_dir) else {
        return std::ptr::null_mut();
    };
    let files_dir: String = files_dir.into();
    let Ok(current_dir) = env.get_string(&current_dir) else {
        return std::ptr::null_mut();
    };
    let current_dir: String = current_dir.into();
    let Ok(command) = env.get_string(&command) else {
        return std::ptr::null_mut();
    };
    let command: String = command.into();
    let result = match Terminal::new(&files_dir, Some(Path::new(&current_dir))) {
        Ok(mut terminal) => {
            let result = terminal.execute(&command);
            [
                result.exit_code.to_string(),
                result.current_dir.to_string_lossy().into_owned(),
                result.stdout,
                result.stderr,
            ]
        }
        Err(error) => [
            "1".to_owned(),
            PathBuf::from(&current_dir).to_string_lossy().into_owned(),
            String::new(),
            format!("terminal: {error}\n"),
        ],
    };
    let Ok(values) = env.new_object_array(4, "java/lang/String", JObject::null()) else {
        return std::ptr::null_mut();
    };
    for (index, value) in result.iter().enumerate() {
        let Ok(value) = env.new_string(value) else {
            return std::ptr::null_mut();
        };
        if env
            .set_object_array_element(&values, index as i32, value)
            .is_err()
        {
            return std::ptr::null_mut();
        }
    }
    values.into_raw()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_pocketterm_android_MainActivity_cancelCommand(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jboolean {
    if cancel_running_process() { 1 } else { 0 }
}
