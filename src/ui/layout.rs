use ratatui::layout::Rect;

pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 15;
pub const STATUS_HEIGHT: u16 = 3;
pub const COMPOSE_HEIGHT: u16 = 3;

#[derive(Debug, Clone, Copy)]
pub struct Areas {
    pub list: Rect,
    pub compose: Option<Rect>,
    pub status: Rect,
}

pub fn split(area: Rect, has_compose: bool) -> Option<Areas> {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return None;
    }
    let compose_height = if has_compose { COMPOSE_HEIGHT } else { 0 };
    let list_height = area.height.checked_sub(STATUS_HEIGHT + compose_height)?;
    if list_height < 3 {
        return None;
    }
    let list = Rect::new(area.x, area.y, area.width, list_height);
    let compose =
        has_compose.then(|| Rect::new(area.x, area.y + list_height, area.width, COMPOSE_HEIGHT));
    let status = Rect::new(
        area.x,
        area.y + list_height + compose_height,
        area.width,
        STATUS_HEIGHT,
    );
    Some(Areas {
        list,
        compose,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_too_small_terminals() {
        assert!(split(Rect::new(0, 0, 40, 10), false).is_none());
        assert!(split(Rect::new(0, 0, 60, 14), false).is_none());
    }

    #[test]
    fn splits_list_status_and_compose() {
        let areas = split(Rect::new(0, 0, 80, 24), true).unwrap();
        assert_eq!(areas.list.height, 18);
        assert_eq!(areas.compose.unwrap().height, 3);
        assert_eq!(areas.status.height, 3);
        assert_eq!(areas.status.y, 21);
    }
}
