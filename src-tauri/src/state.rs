use crate::{
    models::{AppState, AppStore},
    steam,
};

pub(crate) fn build_state(store: AppStore) -> Result<AppState, String> {
    steam::ensure_opensteamtool_aligned(&store);

    Ok(AppState {
        install_status: steam::install_status(&store),
        package_sync_supported: steam::supports_package_sync(),
        steam_client: steam::client_status(&store),
        settings: store.settings,
        packages: store.packages,
        tickets: store.tickets,
    })
}
