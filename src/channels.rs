use crate::models::ProgrammeType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelGroup {
    National,
    Regional,
    Local,
}

#[derive(Debug, Clone)]
pub struct ChannelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub group: ChannelGroup,
    pub prog_type: ProgrammeType,
}

pub static TV_CHANNELS: &[ChannelDef] = &[
    // National
    ChannelDef { id: "p00fzl6b", name: "BBC Four", group: ChannelGroup::National, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6g", name: "BBC News", group: ChannelGroup::National, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl73", name: "BBC Parliament", group: ChannelGroup::National, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl95", name: "BBC Three", group: ChannelGroup::National, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p015pksy", name: "BBC Two", group: ChannelGroup::National, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl9r", name: "CBBC", group: ChannelGroup::National, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl9s", name: "CBeebies", group: ChannelGroup::National, prog_type: ProgrammeType::Tv },

    // Regional
    ChannelDef { id: "p00fzl67", name: "BBC Alba", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6q", name: "BBC One Northern Ireland", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00zskxc", name: "BBC One Northern Ireland HD", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6v", name: "BBC One Scotland", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p013blmc", name: "BBC One Scotland HD", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6z", name: "BBC One Wales", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p013bkc7", name: "BBC One Wales HD", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p06kvypx", name: "BBC Scotland", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p06p396y", name: "BBC Scotland HD", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl97", name: "BBC Two England", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl99", name: "BBC Two Northern Ireland", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p06ngcbm", name: "BBC Two Northern Ireland HD", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl9d", name: "BBC Two Wales", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p06ngc52", name: "BBC Two Wales HD", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p020dmkf", name: "S4C", group: ChannelGroup::Regional, prog_type: ProgrammeType::Tv },

    // Local
    ChannelDef { id: "p00fzl6h", name: "BBC One Cambridgeshire", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6j", name: "BBC One Channel Islands", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6k", name: "BBC One East", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6l", name: "BBC One East Midlands", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6m", name: "BBC One Yorks & Lincs", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6p", name: "BBC One London", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6r", name: "BBC One North East & Cumbria", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6s", name: "BBC One North West", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6t", name: "BBC One Oxfordshire", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6w", name: "BBC One South", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6x", name: "BBC One South East", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl6y", name: "BBC One South West", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl70", name: "BBC One West", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl71", name: "BBC One West Midlands", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
    ChannelDef { id: "p00fzl72", name: "BBC One Yorkshire", group: ChannelGroup::Local, prog_type: ProgrammeType::Tv },
];

pub static RADIO_CHANNELS: &[ChannelDef] = &[
    // National
    ChannelDef { id: "p00fzl64", name: "BBC Radio 1Xtra", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7g", name: "BBC Radio 5 live", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7h", name: "BBC Radio 5 live sports extra", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl65", name: "BBC Radio 6 Music", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl68", name: "BBC Asian Network", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl86", name: "BBC Radio 1", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8v", name: "BBC Radio 2", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8t", name: "BBC Radio 3", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7j", name: "BBC Radio 4", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7l", name: "BBC Radio 4 Extra", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p02zbmb3", name: "BBC World Service", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p02jf21y", name: "CBeebies Radio", group: ChannelGroup::National, prog_type: ProgrammeType::Radio },

    // Regional
    ChannelDef { id: "p00fzl7b", name: "BBC Radio Cymru", group: ChannelGroup::Regional, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7m", name: "BBC Radio Foyle", group: ChannelGroup::Regional, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl81", name: "BBC Radio Nan Gaidheal", group: ChannelGroup::Regional, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8d", name: "BBC Radio Scotland", group: ChannelGroup::Regional, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8w", name: "BBC Radio Ulster", group: ChannelGroup::Regional, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8y", name: "BBC Radio Wales", group: ChannelGroup::Regional, prog_type: ProgrammeType::Radio },

    // Local
    ChannelDef { id: "p00fzl78", name: "BBC Coventry & Warwickshire", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7f", name: "BBC Essex", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7q", name: "BBC Hereford & Worcester", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl82", name: "BBC Newcastle", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8m", name: "BBC Somerset", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8q", name: "BBC Surrey", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8r", name: "BBC Sussex", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl93", name: "BBC Tees", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8z", name: "BBC Wiltshire", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl74", name: "BBC Radio Berkshire", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl75", name: "BBC Radio Bristol", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl76", name: "BBC Radio Cambridgeshire", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl77", name: "BBC Radio Cornwall", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl79", name: "BBC Radio Cumbria", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7c", name: "BBC Radio Derby", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7d", name: "BBC Radio Devon", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7n", name: "BBC Radio Gloucestershire", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7p", name: "BBC Radio Guernsey", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7r", name: "BBC Radio Humberside", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7s", name: "BBC Radio Jersey", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7t", name: "BBC Radio Kent", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7v", name: "BBC Radio Lancashire", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7w", name: "BBC Radio Leeds", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7x", name: "BBC Radio Leicester", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7y", name: "BBC Radio Lincolnshire", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl6f", name: "BBC Radio London", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl7z", name: "BBC Radio Manchester", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl80", name: "BBC Radio Merseyside", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl83", name: "BBC Radio Norfolk", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl84", name: "BBC Radio Northampton", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl85", name: "BBC Radio Nottingham", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8c", name: "BBC Radio Oxford", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8h", name: "BBC Radio Sheffield", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8k", name: "BBC Radio Shropshire", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8l", name: "BBC Radio Solent", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8n", name: "BBC Radio Stoke", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl8p", name: "BBC Radio Suffolk", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl90", name: "BBC Radio York", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl96", name: "BBC Three Counties Radio", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
    ChannelDef { id: "p00fzl9f", name: "BBC WM 95.6", group: ChannelGroup::Local, prog_type: ProgrammeType::Radio },
];

/// Collapse regional BBC One/Two channels into "BBC One" or "BBC Two" for caching
pub fn collapse_channel_name(channel: &str, prog_type: ProgrammeType) -> String {
    if prog_type == ProgrammeType::Tv {
        if channel.starts_with("BBC One") {
            return "BBC One".to_string();
        }
        if channel.starts_with("BBC Two") {
            return "BBC Two".to_string();
        }
    }
    channel.to_string()
}

pub fn get_channels(prog_type: ProgrammeType) -> Vec<&'static ChannelDef> {
    match prog_type {
        ProgrammeType::Tv => TV_CHANNELS.iter().collect(),
        ProgrammeType::Radio => RADIO_CHANNELS.iter().collect(),
        ProgrammeType::All => TV_CHANNELS.iter().chain(RADIO_CHANNELS.iter()).collect(),
    }
}
