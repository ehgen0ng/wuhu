#pragma once
#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

bool Steam_Init();
bool RequestEncryptedAppTicket();
int GetEncryptedAppTicket(void* buf, int bufSize);
uint64_t GetSteamID();

#ifdef __cplusplus
}
#endif
