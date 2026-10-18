package ru.bashkir.words;
import static org.junit.Assert.*;
import org.junit.Test;
import java.util.Calendar;
import java.util.TimeZone;

public class DailyGiftScheduleTest {
    private Calendar at(String zone, int year, int month, int day, int hour, int minute) {
        Calendar date=Calendar.getInstance(TimeZone.getTimeZone(zone));date.clear();date.set(year,month-1,day,hour,minute);return date;
    }
    @Test public void beforeNoonSchedulesTodayAndAtNoonSchedulesTomorrow() {
        Calendar morning=at("Europe/Moscow",2026,10,19,11,59);
        assertEquals(at("Europe/Moscow",2026,10,19,12,0).getTimeInMillis(),DailyGiftSchedule.nextNoon(morning.getTimeInMillis(),morning.getTimeZone()));
        Calendar noon=at("Europe/Moscow",2026,10,19,12,0);
        assertEquals(at("Europe/Moscow",2026,10,20,12,0).getTimeInMillis(),DailyGiftSchedule.nextNoon(noon.getTimeInMillis(),noon.getTimeZone()));
    }
    @Test public void daylightSavingAndTimeZoneChangesKeepLocalNoon() {
        Calendar before=at("Europe/Berlin",2026,10,24,13,0);
        long next=DailyGiftSchedule.nextNoon(before.getTimeInMillis(),before.getTimeZone());
        assertEquals(at("Europe/Berlin",2026,10,25,12,0).getTimeInMillis(),next);
        assertEquals(at("Europe/Moscow",2026,10,25,12,0).getTimeInMillis(),DailyGiftSchedule.nextNoon(before.getTimeInMillis(),TimeZone.getTimeZone("Europe/Moscow")));
    }
    @Test public void localClaimDateMatchesJavascriptEpochDayAcrossUtcMidnight() {
        Calendar local=at("Europe/Moscow",2026,10,19,0,15);
        assertEquals(at("UTC",2026,10,19,0,0).getTimeInMillis()/86400000L,DailyGiftSchedule.localDay(local));
        assertEquals(DailyGiftSchedule.localDay(local),DailyGiftSchedule.localDay(at("Europe/Moscow",2026,10,19,23,59)));
    }
}
