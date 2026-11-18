use ratatui::layout::Rect;

pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 15;
pub const STATUS_HEIGHT: u16 = 3;
pub const COMPOSE_HEIGHT: u16 = 3;
pub const MEMBERS_WIDTH: u16 = 26;

#[derive(Debug, Clone, Copy)]
pub struct Areas {
    pub list: Rect,
    pub compose: Option<Rect>,
    pub members: Option<Rect>,
    pub status: Rect,
}

pub fn split(area: Rect, has_compose: bool, has_members: bool) -> Option<Areas> {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return None;
    }
    let compose_height = if has_compose { COMPOSE_HEIGHT } else { 0 };
    let list_height = area.height.checked_sub(STATUS_HEIGHT + compose_height)?;
    if list_height < 3 {
        return None;
    }
    let members = has_members.then(|| {
        let width = MEMBERS_WIDTH
            .min(area.width / 3)
            .max(18)
            .min(area.width.saturating_sub(30));
        Rect::new(
            area.x + area.width.saturating_sub(width),
            area.y,
            width,
            list_height + compose_height,
        )
    });
    let list_width = members
        .map(|members| area.width.saturating_sub(members.width))
        .unwrap_or(area.width);
    let list = Rect::new(area.x, area.y, list_width, list_height);
    let compose =
        has_compose.then(|| Rect::new(area.x, area.y + list_height, list_width, COMPOSE_HEIGHT));
    let status = Rect::new(
        area.x,
        area.y + list_height + compose_height,
        area.width,
        STATUS_HEIGHT,
    );
    Some(Areas {
        list,
        compose,
        members,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_too_small_terminals() {
        assert!(split(Rect::new(0, 0, 40, 10), false, false).is_none());
        assert!(split(Rect::new(0, 0, 60, 14), false, false).is_none());
    }

    #[test]
    fn splits_list_status_and_compose() {
        let areas = split(Rect::new(0, 0, 80, 24), true, false).unwrap();
        assert_eq!(areas.list.height, 18);
        assert_eq!(areas.compose.unwrap().height, 3);
        assert_eq!(areas.status.height, 3);
        assert_eq!(areas.status.y, 21);
        assert!(areas.members.is_none());
    }

    #[test]
    fn splits_members_sidebar() {
        let areas = split(Rect::new(0, 0, 100, 24), false, true).unwrap();
        let members = areas.members.unwrap();
        assert_eq!(members.width, 26);
        assert_eq!(areas.list.width, 74);
        assert_eq!(members.y, 0);
        assert_eq!(members.height, 21);
    }
}
