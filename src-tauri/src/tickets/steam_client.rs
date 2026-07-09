use std::{ffi::c_void, mem, ptr, thread, time::Duration};

pub(crate) struct ExtractedTickets {
    pub(crate) app_ticket: Option<Vec<u8>>,
    pub(crate) e_ticket: Option<Vec<u8>>,
}

type SteamApiCall = u64;

const STEAM_CLIENT_INTERFACE_VERSION: &[u8] = b"SteamClient023\0";
const STEAM_USER_INTERFACE_VERSION: &[u8] = b"SteamUser023\0";
const STEAM_UTILS_INTERFACE_VERSION: &[u8] = b"SteamUtils010\0";
const STEAM_APP_TICKET_INTERFACE_VERSION: &[u8] = b"STEAMAPPTICKET_INTERFACE_VERSION001\0";
const ENCRYPTED_APP_TICKET_CALLBACK: i32 = 154;
const ERESULT_OK: i32 = 1;
const APP_TICKET_BUFFER_SIZE: usize = 2048;
const E_TICKET_POLL_ATTEMPTS: usize = 300;
const E_TICKET_POLL_STEP: Duration = Duration::from_millis(50);

#[repr(C)]
struct EncryptedAppTicketResponse {
    result: i32,
}

trait SteamTicketSession {
    fn app_ticket_interface(&self) -> *mut c_void;

    fn get_app_ownership_ticket_data(
        &self,
        app_ticket: *mut c_void,
        app_id: u32,
        buffer: *mut c_void,
        buffer_len: u32,
        app_id_offset: *mut u32,
        steam_id_offset: *mut u32,
        signature_offset: *mut u32,
        signature_size: *mut u32,
    ) -> u32;

    fn utils_interface(&self) -> *mut c_void;
    fn user_interface(&self) -> *mut c_void;
    fn request_encrypted_app_ticket(&self, steam_user: *mut c_void) -> SteamApiCall;
    fn is_api_call_completed(&self, utils: *mut c_void, call: SteamApiCall, failed: *mut u8) -> u8;
    fn get_api_call_result(
        &self,
        utils: *mut c_void,
        call: SteamApiCall,
        response: *mut c_void,
        response_size: i32,
        callback_expected: i32,
        failed: *mut u8,
    ) -> u8;
    fn get_encrypted_app_ticket(
        &self,
        steam_user: *mut c_void,
        ticket: *mut c_void,
        ticket_len: i32,
        ticket_size: *mut u32,
    ) -> u8;
}

fn extract_from_session<S: SteamTicketSession>(
    session: &S,
    app_id: u32,
) -> Result<ExtractedTickets, String> {
    Ok(ExtractedTickets {
        app_ticket: extract_app_ticket(session, app_id)?,
        e_ticket: extract_e_ticket(session)?,
    })
}

fn extract_app_ticket<S: SteamTicketSession>(
    session: &S,
    app_id: u32,
) -> Result<Option<Vec<u8>>, String> {
    let app_ticket = session.app_ticket_interface();
    if app_ticket.is_null() {
        return Ok(None);
    }

    let mut buffer = vec![0u8; APP_TICKET_BUFFER_SIZE];
    let mut app_id_offset = 0;
    let mut steam_id_offset = 0;
    let mut signature_offset = 0;
    let mut signature_size = 0;
    let written = session.get_app_ownership_ticket_data(
        app_ticket,
        app_id,
        buffer.as_mut_ptr() as *mut c_void,
        buffer.len() as u32,
        &mut app_id_offset,
        &mut steam_id_offset,
        &mut signature_offset,
        &mut signature_size,
    );
    if written == 0 {
        return Ok(None);
    }
    if written as usize > buffer.len() {
        return Err("AppTicket 大小超出缓冲区".to_string());
    }
    buffer.truncate(written as usize);
    Ok(Some(buffer))
}

fn extract_e_ticket<S: SteamTicketSession>(session: &S) -> Result<Option<Vec<u8>>, String> {
    let utils = session.utils_interface();
    let steam_user = session.user_interface();
    if utils.is_null() || steam_user.is_null() {
        return Ok(None);
    }

    let call = session.request_encrypted_app_ticket(steam_user);
    if call == 0 {
        return Ok(None);
    }

    let mut failed = 0u8;
    for _ in 0..E_TICKET_POLL_ATTEMPTS {
        if session.is_api_call_completed(utils, call, &mut failed) != 0 {
            break;
        }
        thread::sleep(E_TICKET_POLL_STEP);
    }

    if failed != 0 {
        return Ok(None);
    }

    let mut response = EncryptedAppTicketResponse { result: 0 };
    let got_result = session.get_api_call_result(
        utils,
        call,
        &mut response as *mut _ as *mut c_void,
        mem::size_of::<EncryptedAppTicketResponse>() as i32,
        ENCRYPTED_APP_TICKET_CALLBACK,
        &mut failed,
    );
    if got_result == 0 || failed != 0 || response.result != ERESULT_OK {
        return Ok(None);
    }

    let mut ticket_size = 0u32;
    session.get_encrypted_app_ticket(steam_user, ptr::null_mut(), 0, &mut ticket_size);
    if ticket_size == 0 {
        return Ok(None);
    }

    let mut buffer = vec![0u8; ticket_size as usize];
    let ok = session.get_encrypted_app_ticket(
        steam_user,
        buffer.as_mut_ptr() as *mut c_void,
        buffer.len() as i32,
        &mut ticket_size,
    );
    if ok == 0 {
        return Ok(None);
    }
    buffer.truncate(ticket_size as usize);
    Ok(Some(buffer))
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn extract(
    _steam_root: &std::path::Path,
    _app_id: u32,
) -> Result<ExtractedTickets, String> {
    Err("Ticket 提取目前只支持 Windows 和 macOS Steam 客户端".to_string())
}

#[cfg(windows)]
mod windows {
    use super::{
        c_void, extract_from_session, ExtractedTickets, SteamApiCall, SteamTicketSession,
        STEAM_APP_TICKET_INTERFACE_VERSION, STEAM_CLIENT_INTERFACE_VERSION,
        STEAM_USER_INTERFACE_VERSION, STEAM_UTILS_INTERFACE_VERSION,
    };
    use std::{
        ffi::{c_char, CString, OsStr},
        mem,
        os::windows::ffi::OsStrExt,
        path::Path,
        ptr,
    };

    type HModule = *mut c_void;
    type HSteamPipe = i32;
    type HSteamUser = i32;

    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetDllDirectoryW(path_name: *const u16) -> i32;
        fn LoadLibraryExW(file_name: *const u16, file: *mut c_void, flags: u32) -> HModule;
        fn GetProcAddress(module: HModule, proc_name: *const c_char) -> *mut c_void;
        fn FreeLibrary(module: HModule) -> i32;
        fn GetLastError() -> u32;
    }

    #[repr(C)]
    struct ISteamClient {
        vtable: *const ISteamClientVtbl,
    }

    #[repr(C)]
    struct ISteamClientVtbl {
        create_steam_pipe: unsafe extern "system" fn(*mut ISteamClient) -> HSteamPipe,
        b_release_steam_pipe: unsafe extern "system" fn(*mut ISteamClient, HSteamPipe) -> u8,
        connect_to_global_user:
            unsafe extern "system" fn(*mut ISteamClient, HSteamPipe) -> HSteamUser,
        create_local_user:
            unsafe extern "system" fn(*mut ISteamClient, *mut HSteamPipe, i32) -> HSteamUser,
        release_user: unsafe extern "system" fn(*mut ISteamClient, HSteamPipe, HSteamUser),
        get_isteam_user: unsafe extern "system" fn(
            *mut ISteamClient,
            HSteamUser,
            HSteamPipe,
            *const c_char,
        ) -> *mut ISteamUser,
        get_isteam_game_server: usize,
        set_local_ip_binding: usize,
        get_isteam_friends: usize,
        get_isteam_utils: unsafe extern "system" fn(
            *mut ISteamClient,
            HSteamPipe,
            *const c_char,
        ) -> *mut ISteamUtils,
        get_isteam_matchmaking: usize,
        get_isteam_matchmaking_servers: usize,
        get_isteam_generic_interface: unsafe extern "system" fn(
            *mut ISteamClient,
            HSteamUser,
            HSteamPipe,
            *const c_char,
        ) -> *mut c_void,
    }

    #[repr(C)]
    struct ISteamUtils {
        vtable: *const ISteamUtilsVtbl,
    }

    #[repr(C)]
    struct ISteamUtilsVtbl {
        get_seconds_since_app_active: usize,
        get_seconds_since_computer_active: usize,
        get_connected_universe: usize,
        get_server_real_time: usize,
        get_ip_country: usize,
        get_image_size: usize,
        get_image_rgba: usize,
        get_cser_ip_port: usize,
        get_current_battery_power: usize,
        get_app_id: usize,
        set_overlay_notification_position: usize,
        is_api_call_completed:
            unsafe extern "system" fn(*mut ISteamUtils, SteamApiCall, *mut u8) -> u8,
        get_api_call_failure_reason: usize,
        get_api_call_result: unsafe extern "system" fn(
            *mut ISteamUtils,
            SteamApiCall,
            *mut c_void,
            i32,
            i32,
            *mut u8,
        ) -> u8,
    }

    #[repr(C)]
    struct ISteamUser {
        vtable: *const ISteamUserVtbl,
    }

    #[repr(C)]
    struct ISteamUserVtbl {
        get_h_steam_user: usize,
        b_logged_on: usize,
        get_steam_id: usize,
        initiate_game_connection_deprecated: usize,
        terminate_game_connection_deprecated: usize,
        track_app_usage_event: usize,
        get_user_data_folder: usize,
        start_voice_recording: usize,
        stop_voice_recording: usize,
        get_available_voice: usize,
        get_voice: usize,
        decompress_voice: usize,
        get_voice_optimal_sample_rate: usize,
        get_auth_session_ticket: usize,
        get_auth_ticket_for_web_api: usize,
        begin_auth_session: usize,
        end_auth_session: usize,
        cancel_auth_ticket: usize,
        user_has_license_for_app: usize,
        b_is_behind_nat: usize,
        advertise_game: usize,
        request_encrypted_app_ticket:
            unsafe extern "system" fn(*mut ISteamUser, *mut c_void, i32) -> SteamApiCall,
        get_encrypted_app_ticket:
            unsafe extern "system" fn(*mut ISteamUser, *mut c_void, i32, *mut u32) -> u8,
    }

    #[repr(C)]
    struct ISteamAppTicket {
        vtable: *const ISteamAppTicketVtbl,
    }

    #[repr(C)]
    struct ISteamAppTicketVtbl {
        get_app_ownership_ticket_data: unsafe extern "system" fn(
            *mut ISteamAppTicket,
            u32,
            *mut c_void,
            u32,
            *mut u32,
            *mut u32,
            *mut u32,
            *mut u32,
        ) -> u32,
    }

    type CreateInterfaceFn = unsafe extern "system" fn(*const c_char, *mut i32) -> *mut c_void;

    struct SteamClientLibrary {
        module: HModule,
    }

    impl Drop for SteamClientLibrary {
        fn drop(&mut self) {
            unsafe {
                FreeLibrary(self.module);
            }
        }
    }

    struct SteamSession {
        client: *mut ISteamClient,
        pipe: HSteamPipe,
        user: HSteamUser,
    }

    impl Drop for SteamSession {
        fn drop(&mut self) {
            if !self.client.is_null() && self.pipe != 0 {
                unsafe {
                    ((*(*self.client).vtable).b_release_steam_pipe)(self.client, self.pipe);
                }
            }
        }
    }

    pub(crate) fn extract(steam_root: &Path, app_id: u32) -> Result<ExtractedTickets, String> {
        if app_id == 0 {
            return Err("AppID 无效".to_string());
        }

        let steam_client_path = steam_root.join("steamclient64.dll");
        if !steam_client_path.exists() {
            return Err("Steam 根目录下没有找到 steamclient64.dll".to_string());
        }

        std::env::set_var("SteamAppId", app_id.to_string());
        std::env::set_var("SteamGameId", app_id.to_string());

        let library = load_steam_client(steam_root, &steam_client_path)?;
        let client = create_steam_client(&library)?;
        let session = open_session(client)?;

        extract_from_session(&session, app_id)
    }

    fn load_steam_client(
        steam_root: &Path,
        steam_client_path: &Path,
    ) -> Result<SteamClientLibrary, String> {
        unsafe {
            let root_wide = wide_null(steam_root.as_os_str());
            SetDllDirectoryW(root_wide.as_ptr());

            let dll_wide = wide_null(steam_client_path.as_os_str());
            let module = LoadLibraryExW(
                dll_wide.as_ptr(),
                ptr::null_mut(),
                LOAD_WITH_ALTERED_SEARCH_PATH,
            );
            if module.is_null() {
                return Err(format!(
                    "加载 steamclient64.dll 失败，Windows 错误码 {}",
                    GetLastError()
                ));
            }
            Ok(SteamClientLibrary { module })
        }
    }

    fn create_steam_client(library: &SteamClientLibrary) -> Result<*mut ISteamClient, String> {
        unsafe {
            let symbol = CString::new("CreateInterface").unwrap();
            let ptr = GetProcAddress(library.module, symbol.as_ptr());
            if ptr.is_null() {
                return Err("steamclient64.dll 缺少 CreateInterface 导出".to_string());
            }
            let create_interface: CreateInterfaceFn = mem::transmute(ptr);
            let mut return_code = 0;
            let client = create_interface(
                STEAM_CLIENT_INTERFACE_VERSION.as_ptr() as *const c_char,
                &mut return_code,
            ) as *mut ISteamClient;
            if client.is_null() {
                return Err(format!(
                    "CreateInterface(SteamClient023) 失败，返回码 {return_code}"
                ));
            }
            Ok(client)
        }
    }

    fn open_session(client: *mut ISteamClient) -> Result<SteamSession, String> {
        unsafe {
            let pipe = ((*(*client).vtable).create_steam_pipe)(client);
            if pipe == 0 {
                return Err("CreateSteamPipe 失败，请确认 Steam 正在运行".to_string());
            }

            let user = ((*(*client).vtable).connect_to_global_user)(client, pipe);
            if user == 0 {
                ((*(*client).vtable).b_release_steam_pipe)(client, pipe);
                return Err("ConnectToGlobalUser 失败，请确认 Steam 已登录账号".to_string());
            }

            Ok(SteamSession { client, pipe, user })
        }
    }

    impl SteamTicketSession for SteamSession {
        fn app_ticket_interface(&self) -> *mut c_void {
            unsafe {
                ((*(*self.client).vtable).get_isteam_generic_interface)(
                    self.client,
                    self.user,
                    self.pipe,
                    STEAM_APP_TICKET_INTERFACE_VERSION.as_ptr() as *const c_char,
                )
            }
        }

        fn get_app_ownership_ticket_data(
            &self,
            app_ticket: *mut c_void,
            app_id: u32,
            buffer: *mut c_void,
            buffer_len: u32,
            app_id_offset: *mut u32,
            steam_id_offset: *mut u32,
            signature_offset: *mut u32,
            signature_size: *mut u32,
        ) -> u32 {
            let app_ticket = app_ticket as *mut ISteamAppTicket;
            unsafe {
                ((*(*app_ticket).vtable).get_app_ownership_ticket_data)(
                    app_ticket,
                    app_id,
                    buffer,
                    buffer_len,
                    app_id_offset,
                    steam_id_offset,
                    signature_offset,
                    signature_size,
                )
            }
        }

        fn utils_interface(&self) -> *mut c_void {
            unsafe {
                ((*(*self.client).vtable).get_isteam_utils)(
                    self.client,
                    self.pipe,
                    STEAM_UTILS_INTERFACE_VERSION.as_ptr() as *const c_char,
                ) as *mut c_void
            }
        }

        fn user_interface(&self) -> *mut c_void {
            unsafe {
                ((*(*self.client).vtable).get_isteam_user)(
                    self.client,
                    self.user,
                    self.pipe,
                    STEAM_USER_INTERFACE_VERSION.as_ptr() as *const c_char,
                ) as *mut c_void
            }
        }

        fn request_encrypted_app_ticket(&self, steam_user: *mut c_void) -> SteamApiCall {
            let steam_user = steam_user as *mut ISteamUser;
            unsafe {
                ((*(*steam_user).vtable).request_encrypted_app_ticket)(
                    steam_user,
                    std::ptr::null_mut(),
                    0,
                )
            }
        }

        fn is_api_call_completed(
            &self,
            utils: *mut c_void,
            call: SteamApiCall,
            failed: *mut u8,
        ) -> u8 {
            let utils = utils as *mut ISteamUtils;
            unsafe { ((*(*utils).vtable).is_api_call_completed)(utils, call, failed) }
        }

        fn get_api_call_result(
            &self,
            utils: *mut c_void,
            call: SteamApiCall,
            response: *mut c_void,
            response_size: i32,
            callback_expected: i32,
            failed: *mut u8,
        ) -> u8 {
            let utils = utils as *mut ISteamUtils;
            unsafe {
                ((*(*utils).vtable).get_api_call_result)(
                    utils,
                    call,
                    response,
                    response_size,
                    callback_expected,
                    failed,
                )
            }
        }

        fn get_encrypted_app_ticket(
            &self,
            steam_user: *mut c_void,
            ticket: *mut c_void,
            ticket_len: i32,
            ticket_size: *mut u32,
        ) -> u8 {
            let steam_user = steam_user as *mut ISteamUser;
            unsafe {
                ((*(*steam_user).vtable).get_encrypted_app_ticket)(
                    steam_user,
                    ticket,
                    ticket_len,
                    ticket_size,
                )
            }
        }
    }

    fn wide_null(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }
}

#[cfg(windows)]
pub(crate) use windows::extract;

#[cfg(target_os = "macos")]
mod macos {
    use super::{
        c_void, extract_from_session, ExtractedTickets, SteamApiCall, SteamTicketSession,
        STEAM_APP_TICKET_INTERFACE_VERSION, STEAM_CLIENT_INTERFACE_VERSION,
        STEAM_USER_INTERFACE_VERSION, STEAM_UTILS_INTERFACE_VERSION,
    };
    use std::{
        ffi::{c_char, c_int, CStr, CString},
        mem,
        os::unix::ffi::OsStrExt,
        path::Path,
    };

    type HModule = *mut c_void;
    type HSteamPipe = i32;
    type HSteamUser = i32;

    const RTLD_NOW: c_int = 0x2;

    unsafe extern "C" {
        fn dlopen(path: *const c_char, mode: c_int) -> HModule;
        fn dlsym(handle: HModule, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: HModule) -> c_int;
        fn dlerror() -> *const c_char;
    }

    #[repr(C)]
    struct ISteamClient {
        vtable: *const ISteamClientVtbl,
    }

    #[repr(C)]
    struct ISteamClientVtbl {
        create_steam_pipe: unsafe extern "C" fn(*mut ISteamClient) -> HSteamPipe,
        b_release_steam_pipe: unsafe extern "C" fn(*mut ISteamClient, HSteamPipe) -> u8,
        connect_to_global_user: unsafe extern "C" fn(*mut ISteamClient, HSteamPipe) -> HSteamUser,
        create_local_user:
            unsafe extern "C" fn(*mut ISteamClient, *mut HSteamPipe, i32) -> HSteamUser,
        release_user: unsafe extern "C" fn(*mut ISteamClient, HSteamPipe, HSteamUser),
        get_isteam_user: unsafe extern "C" fn(
            *mut ISteamClient,
            HSteamUser,
            HSteamPipe,
            *const c_char,
        ) -> *mut ISteamUser,
        get_isteam_game_server: usize,
        set_local_ip_binding: usize,
        get_isteam_friends: usize,
        get_isteam_utils:
            unsafe extern "C" fn(*mut ISteamClient, HSteamPipe, *const c_char) -> *mut ISteamUtils,
        get_isteam_matchmaking: usize,
        get_isteam_matchmaking_servers: usize,
        get_isteam_generic_interface: unsafe extern "C" fn(
            *mut ISteamClient,
            HSteamUser,
            HSteamPipe,
            *const c_char,
        ) -> *mut c_void,
    }

    #[repr(C)]
    struct ISteamUtils {
        vtable: *const ISteamUtilsVtbl,
    }

    #[repr(C)]
    struct ISteamUtilsVtbl {
        get_seconds_since_app_active: usize,
        get_seconds_since_computer_active: usize,
        get_connected_universe: usize,
        get_server_real_time: usize,
        get_ip_country: usize,
        get_image_size: usize,
        get_image_rgba: usize,
        get_cser_ip_port: usize,
        get_current_battery_power: usize,
        get_app_id: usize,
        set_overlay_notification_position: usize,
        is_api_call_completed: unsafe extern "C" fn(*mut ISteamUtils, SteamApiCall, *mut u8) -> u8,
        get_api_call_failure_reason: usize,
        get_api_call_result: unsafe extern "C" fn(
            *mut ISteamUtils,
            SteamApiCall,
            *mut c_void,
            i32,
            i32,
            *mut u8,
        ) -> u8,
    }

    #[repr(C)]
    struct ISteamUser {
        vtable: *const ISteamUserVtbl,
    }

    #[repr(C)]
    struct ISteamUserVtbl {
        get_h_steam_user: usize,
        b_logged_on: usize,
        get_steam_id: usize,
        initiate_game_connection_deprecated: usize,
        terminate_game_connection_deprecated: usize,
        track_app_usage_event: usize,
        get_user_data_folder: usize,
        start_voice_recording: usize,
        stop_voice_recording: usize,
        get_available_voice: usize,
        get_voice: usize,
        decompress_voice: usize,
        get_voice_optimal_sample_rate: usize,
        get_auth_session_ticket: usize,
        get_auth_ticket_for_web_api: usize,
        begin_auth_session: usize,
        end_auth_session: usize,
        cancel_auth_ticket: usize,
        user_has_license_for_app: usize,
        b_is_behind_nat: usize,
        advertise_game: usize,
        request_encrypted_app_ticket:
            unsafe extern "C" fn(*mut ISteamUser, *mut c_void, i32) -> SteamApiCall,
        get_encrypted_app_ticket:
            unsafe extern "C" fn(*mut ISteamUser, *mut c_void, i32, *mut u32) -> u8,
    }

    #[repr(C)]
    struct ISteamAppTicket {
        vtable: *const ISteamAppTicketVtbl,
    }

    #[repr(C)]
    struct ISteamAppTicketVtbl {
        get_app_ownership_ticket_data: unsafe extern "C" fn(
            *mut ISteamAppTicket,
            u32,
            *mut c_void,
            u32,
            *mut u32,
            *mut u32,
            *mut u32,
            *mut u32,
        ) -> u32,
    }

    type CreateInterfaceFn = unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void;

    struct SteamClientLibrary {
        module: HModule,
    }

    impl Drop for SteamClientLibrary {
        fn drop(&mut self) {
            unsafe {
                dlclose(self.module);
            }
        }
    }

    struct SteamSession {
        client: *mut ISteamClient,
        pipe: HSteamPipe,
        user: HSteamUser,
    }

    impl Drop for SteamSession {
        fn drop(&mut self) {
            if !self.client.is_null() && self.pipe != 0 {
                unsafe {
                    ((*(*self.client).vtable).b_release_steam_pipe)(self.client, self.pipe);
                }
            }
        }
    }

    pub(crate) fn extract(steam_root: &Path, app_id: u32) -> Result<ExtractedTickets, String> {
        if app_id == 0 {
            return Err("AppID 无效".to_string());
        }

        let steam_client_path = steam_root
            .join("Steam.AppBundle")
            .join("Steam")
            .join("Contents")
            .join("MacOS")
            .join("steamclient.dylib");
        if !steam_client_path.exists() {
            return Err(
                "Steam 目录下没有找到 Steam.AppBundle/Steam/Contents/MacOS/steamclient.dylib"
                    .to_string(),
            );
        }

        std::env::set_var("SteamAppId", app_id.to_string());
        std::env::set_var("SteamGameId", app_id.to_string());

        let library = load_steam_client(&steam_client_path)?;
        let client = create_steam_client(&library)?;
        let session = open_session(client)?;

        extract_from_session(&session, app_id)
    }

    fn load_steam_client(steam_client_path: &Path) -> Result<SteamClientLibrary, String> {
        unsafe {
            clear_dlerror();
            let dylib_path = CString::new(steam_client_path.as_os_str().as_bytes())
                .map_err(|_| "steamclient.dylib 路径包含无效空字节".to_string())?;
            let module = dlopen(dylib_path.as_ptr(), RTLD_NOW);
            if module.is_null() {
                return Err(format!(
                    "加载 steamclient.dylib 失败：{}",
                    dl_error_message()
                ));
            }
            Ok(SteamClientLibrary { module })
        }
    }

    fn create_steam_client(library: &SteamClientLibrary) -> Result<*mut ISteamClient, String> {
        unsafe {
            clear_dlerror();
            let symbol = CString::new("CreateInterface").unwrap();
            let ptr = dlsym(library.module, symbol.as_ptr());
            if ptr.is_null() {
                return Err(format!(
                    "steamclient.dylib 缺少 CreateInterface 导出：{}",
                    dl_error_message()
                ));
            }
            let create_interface: CreateInterfaceFn = mem::transmute(ptr);
            let mut return_code = 0;
            let client = create_interface(
                STEAM_CLIENT_INTERFACE_VERSION.as_ptr() as *const c_char,
                &mut return_code,
            ) as *mut ISteamClient;
            if client.is_null() {
                return Err(format!(
                    "CreateInterface(SteamClient023) 失败，返回码 {return_code}"
                ));
            }
            Ok(client)
        }
    }

    fn open_session(client: *mut ISteamClient) -> Result<SteamSession, String> {
        unsafe {
            let pipe = ((*(*client).vtable).create_steam_pipe)(client);
            if pipe == 0 {
                return Err("CreateSteamPipe 失败，请确认 Steam 正在运行".to_string());
            }

            let user = ((*(*client).vtable).connect_to_global_user)(client, pipe);
            if user == 0 {
                ((*(*client).vtable).b_release_steam_pipe)(client, pipe);
                return Err(
                    "ConnectToGlobalUser 失败，请确认 Steam 正在运行并已登录账号".to_string(),
                );
            }

            Ok(SteamSession { client, pipe, user })
        }
    }

    impl SteamTicketSession for SteamSession {
        fn app_ticket_interface(&self) -> *mut c_void {
            unsafe {
                ((*(*self.client).vtable).get_isteam_generic_interface)(
                    self.client,
                    self.user,
                    self.pipe,
                    STEAM_APP_TICKET_INTERFACE_VERSION.as_ptr() as *const c_char,
                )
            }
        }

        fn get_app_ownership_ticket_data(
            &self,
            app_ticket: *mut c_void,
            app_id: u32,
            buffer: *mut c_void,
            buffer_len: u32,
            app_id_offset: *mut u32,
            steam_id_offset: *mut u32,
            signature_offset: *mut u32,
            signature_size: *mut u32,
        ) -> u32 {
            let app_ticket = app_ticket as *mut ISteamAppTicket;
            unsafe {
                ((*(*app_ticket).vtable).get_app_ownership_ticket_data)(
                    app_ticket,
                    app_id,
                    buffer,
                    buffer_len,
                    app_id_offset,
                    steam_id_offset,
                    signature_offset,
                    signature_size,
                )
            }
        }

        fn utils_interface(&self) -> *mut c_void {
            unsafe {
                ((*(*self.client).vtable).get_isteam_utils)(
                    self.client,
                    self.pipe,
                    STEAM_UTILS_INTERFACE_VERSION.as_ptr() as *const c_char,
                ) as *mut c_void
            }
        }

        fn user_interface(&self) -> *mut c_void {
            unsafe {
                ((*(*self.client).vtable).get_isteam_user)(
                    self.client,
                    self.user,
                    self.pipe,
                    STEAM_USER_INTERFACE_VERSION.as_ptr() as *const c_char,
                ) as *mut c_void
            }
        }

        fn request_encrypted_app_ticket(&self, steam_user: *mut c_void) -> SteamApiCall {
            let steam_user = steam_user as *mut ISteamUser;
            unsafe {
                ((*(*steam_user).vtable).request_encrypted_app_ticket)(
                    steam_user,
                    std::ptr::null_mut(),
                    0,
                )
            }
        }

        fn is_api_call_completed(
            &self,
            utils: *mut c_void,
            call: SteamApiCall,
            failed: *mut u8,
        ) -> u8 {
            let utils = utils as *mut ISteamUtils;
            unsafe { ((*(*utils).vtable).is_api_call_completed)(utils, call, failed) }
        }

        fn get_api_call_result(
            &self,
            utils: *mut c_void,
            call: SteamApiCall,
            response: *mut c_void,
            response_size: i32,
            callback_expected: i32,
            failed: *mut u8,
        ) -> u8 {
            let utils = utils as *mut ISteamUtils;
            unsafe {
                ((*(*utils).vtable).get_api_call_result)(
                    utils,
                    call,
                    response,
                    response_size,
                    callback_expected,
                    failed,
                )
            }
        }

        fn get_encrypted_app_ticket(
            &self,
            steam_user: *mut c_void,
            ticket: *mut c_void,
            ticket_len: i32,
            ticket_size: *mut u32,
        ) -> u8 {
            let steam_user = steam_user as *mut ISteamUser;
            unsafe {
                ((*(*steam_user).vtable).get_encrypted_app_ticket)(
                    steam_user,
                    ticket,
                    ticket_len,
                    ticket_size,
                )
            }
        }
    }

    unsafe fn clear_dlerror() {
        dlerror();
    }

    unsafe fn dl_error_message() -> String {
        let error = dlerror();
        if error.is_null() {
            return "未知错误".to_string();
        }
        CStr::from_ptr(error).to_string_lossy().into_owned()
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::extract;
