#include "steam_init.h"
#include "steam/steam_api.h"
#include "steam/steam_api_flat.h"

bool Steam_Init() {
    // 使用和 Rust 相同的初始化方式
    SteamErrMsg errMsg;
    ESteamAPIInitResult result = SteamAPI_InitFlat(&errMsg);
    if (result != k_ESteamAPIInitResult_OK) {
        return false;
    }
    
    SteamAPI_ManualDispatch_Init();
    return true;
}

bool RequestEncryptedAppTicket() {
    SteamAPICall_t apiCall = SteamUser()->RequestEncryptedAppTicket(nullptr, 0);
    return apiCall != k_uAPICallInvalid;
}

int GetEncryptedAppTicket(void* buf, int bufSize) {
    uint32 actualSize = 0;
    bool success = SteamUser()->GetEncryptedAppTicket(buf, bufSize, &actualSize);
    return success ? static_cast<int>(actualSize) : 0;
}

uint64 GetSteamID() {
    ISteamUser* user = SteamAPI_SteamUser_v023();
    if (!user) {
        return 0;
    }
    
    return SteamAPI_ISteamUser_GetSteamID(user);
}
