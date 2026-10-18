package ru.bashkir.words;
import java.util.Calendar;
import java.util.TimeZone;

/** Pure calendar calculations shared by reminder delivery and JVM regression tests. */
final class DailyGiftSchedule {
    static long nextNoon(long now, TimeZone zone) {
        Calendar noon = Calendar.getInstance(zone);noon.setTimeInMillis(now);
        noon.set(Calendar.HOUR_OF_DAY, 12);noon.set(Calendar.MINUTE, 0);noon.set(Calendar.SECOND, 0);noon.set(Calendar.MILLISECOND, 0);
        if (noon.getTimeInMillis() <= now) noon.add(Calendar.DATE, 1);
        return noon.getTimeInMillis();
    }
    static long localDay(Calendar date) {
        Calendar utc = Calendar.getInstance(TimeZone.getTimeZone("UTC"));
        utc.clear();utc.set(date.get(Calendar.YEAR), date.get(Calendar.MONTH), date.get(Calendar.DAY_OF_MONTH));
        return utc.getTimeInMillis() / 86400000L;
    }
}
