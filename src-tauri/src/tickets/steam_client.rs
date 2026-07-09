use std::path::Path;

pub(crate) struct ExtractedTickets {
    pub(crate) app_ticket: Option<Vec<u8>>,
    pub(crate) e_ticket: Option<Vec<u8>>,
}

#[cfg(not(windows))]
pub(crate) fn extract(_steam_root: &Path, _app_id: u32) -> Result<ExtractedTickets, String> {
    Err("Ticket 提取只能在 64 位 Windows Steam 环境中使用".to_string())
}

#[cfg(windows)]
mod windows {
    use super::ExtractedTickets;
    use std::{
        ffi::{c_char, c_void, CString, OsStr},
        mem,
        os::windows::ffi::OsStrExt,
        path::Path,
        ptr, thread,
        time::Duration,
    };

    type HModule = *mut c_void;
    type HSteamPipe = i32;
    type HSteamUser = i32;
    type SteamApiCall = u64;

    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;
    const STEAM_CLIENT_INTERFACE_VERSION: &[u8] = b"SteamClient023\0";
    const STEAM_USER_INTERFACE_VERSION: &[u8] = b"SteamUser023\0";
    const STEAM_UTILS_INTERFACE_VERSION: &[u8] = b"SteamUtils010\0";
    const STEAM_APP_TICKET_INTERFACE_VERSION: &[u8] = b"STEAMAPPTICKET_INTERFACE_VERSION001\0";
    const ENCRYPTED_APP_TICKET_CALLBACK: i32 = 154;
    const ERESULT_OK: i32 = 1;

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

    #[repr(C)]
    struct EncryptedAppTicketResponse {
        result: i32,
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

    pub(super) fn extract(steam_root: &Path, app_id: u32) -> Result<ExtractedTickets, String> {
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

        let app_ticket = extract_app_ticket(&session, app_id)?;
        let e_ticket = extract_e_ticket(&session, app_id)?;

        Ok(ExtractedTickets {
            app_ticket,
            e_ticket,
        })
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

    fn extract_app_ticket(session: &SteamSession, app_id: u32) -> Result<Option<Vec<u8>>, String> {
        unsafe {
            let app_ticket = ((*(*session.client).vtable).get_isteam_generic_interface)(
                session.client,
                session.user,
                session.pipe,
                STEAM_APP_TICKET_INTERFACE_VERSION.as_ptr() as *const c_char,
            ) as *mut ISteamAppTicket;
            if app_ticket.is_null() {
                return Ok(None);
            }

            let mut buffer = vec![0u8; 2048];
            let mut app_id_offset = 0;
            let mut steam_id_offset = 0;
            let mut signature_offset = 0;
            let mut signature_size = 0;
            let written = ((*(*app_ticket).vtable).get_app_ownership_ticket_data)(
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
    }

    fn extract_e_ticket(session: &SteamSession, _app_id: u32) -> Result<Option<Vec<u8>>, String> {
        unsafe {
            let utils = ((*(*session.client).vtable).get_isteam_utils)(
                session.client,
                session.pipe,
                STEAM_UTILS_INTERFACE_VERSION.as_ptr() as *const c_char,
            );
            let steam_user = ((*(*session.client).vtable).get_isteam_user)(
                session.client,
                session.user,
                session.pipe,
                STEAM_USER_INTERFACE_VERSION.as_ptr() as *const c_char,
            );
            if utils.is_null() || steam_user.is_null() {
                return Ok(None);
            }

            let call = ((*(*steam_user).vtable).request_encrypted_app_ticket)(
                steam_user,
                ptr::null_mut(),
                0,
            );
            if call == 0 {
                return Ok(None);
            }

            let mut failed = 0u8;
            for _ in 0..300 {
                if ((*(*utils).vtable).is_api_call_completed)(utils, call, &mut failed) != 0 {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }

            if failed != 0 {
                return Ok(None);
            }

            let mut response = EncryptedAppTicketResponse { result: 0 };
            let got_result = ((*(*utils).vtable).get_api_call_result)(
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
            ((*(*steam_user).vtable).get_encrypted_app_ticket)(
                steam_user,
                ptr::null_mut(),
                0,
                &mut ticket_size,
            );
            if ticket_size == 0 {
                return Ok(None);
            }

            let mut buffer = vec![0u8; ticket_size as usize];
            let ok = ((*(*steam_user).vtable).get_encrypted_app_ticket)(
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
    }

    fn wide_null(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }
}

#[cfg(windows)]
pub(crate) use windows::extract;
