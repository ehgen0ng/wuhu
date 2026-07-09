#[derive(Debug, Clone)]
pub(crate) struct TicketData {
    pub(crate) app_id: u32,
    pub(crate) app_ticket: Option<Vec<u8>>,
    pub(crate) e_ticket: Option<Vec<u8>>,
}

pub(crate) fn parse_tickets_txt(content: &str) -> Result<TicketData, String> {
    let mut app_id = None;
    let mut app_ticket = None;
    let mut e_ticket = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((left, right)) = trimmed.split_once(':') else {
            continue;
        };

        let key = left.trim().to_ascii_lowercase();
        let value = right.trim();
        if key == "appid" {
            let parsed = value
                .parse::<u32>()
                .map_err(|_| "tickets.txt 里的 appid 无效".to_string())?;
            if parsed == 0 {
                return Err("tickets.txt 里的 appid 无效".to_string());
            }
            app_id = Some(parsed);
        } else if key.starts_with("appticket") {
            app_ticket = parse_ticket_value("appticket", value)?;
        } else if key.starts_with("eticket") {
            e_ticket = parse_ticket_value("eticket", value)?;
        }
    }

    let app_id = app_id.ok_or_else(|| "tickets.txt 缺少 appid".to_string())?;
    Ok(TicketData {
        app_id,
        app_ticket,
        e_ticket,
    })
}

pub(crate) fn build_tickets_txt(data: &TicketData) -> String {
    let mut output = String::new();
    output.push_str(&format!("appid:{}\n", data.app_id));
    output.push_str(&ticket_line("appticket", data.app_ticket.as_deref()));
    output.push_str(&ticket_line("eticket", data.e_ticket.as_deref()));
    output
}

pub(crate) fn build_lua_lines(data: &TicketData) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(app_ticket) = data.app_ticket.as_deref() {
        lines.push(format!(
            "setAppTicket({}, \"{}\")",
            data.app_id,
            to_hex(app_ticket)
        ));
    }
    if let Some(e_ticket) = data.e_ticket.as_deref() {
        lines.push(format!(
            "setETicket({}, \"{}\")",
            data.app_id,
            to_hex(e_ticket)
        ));
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

pub(crate) fn to_hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(data.len() * 2);
    for byte in data {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn ticket_line(name: &str, data: Option<&[u8]>) -> String {
    match data {
        Some(bytes) => format!("{name}({} bytes):{}\n", bytes.len(), to_hex(bytes)),
        None => format!("{name}:null\n"),
    }
}

fn parse_ticket_value(label: &str, value: &str) -> Result<Option<Vec<u8>>, String> {
    if value.eq_ignore_ascii_case("null") {
        return Ok(None);
    }
    if value.is_empty() {
        return Ok(None);
    }
    hex_to_bytes(value)
        .map(Some)
        .map_err(|err| format!("{label} 无效：{err}"))
}

fn hex_to_bytes(value: &str) -> Result<Vec<u8>, String> {
    let compact: String = value.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.len() % 2 != 0 {
        return Err("十六进制长度必须是偶数".to_string());
    }

    let mut bytes = Vec::with_capacity(compact.len() / 2);
    let raw = compact.as_bytes();
    for pair in raw.chunks_exact(2) {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("包含非十六进制字符".to_string()),
    }
}
