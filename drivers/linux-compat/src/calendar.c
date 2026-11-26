#include <jnox_linux_compat.h>

/* The native frontend admits the ARM counter's unsigned 32-bit Unix range. */
static int leap(int year)
{
    return year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
}

static int month_days(int month, int year)
{
    static const unsigned char days[] = {31,28,31,30,31,30,31,31,30,31,30,31};
    return days[month] + (month == 1 && leap(year));
}

int rtc_year_days(int day, int month, int year)
{
    int days = day - 1;
    for (int i = 0; i < month; ++i)
        days += month_days(i, year);
    return days;
}

time64_t rtc_tm_to_time64(struct rtc_time *tm)
{
    int year = tm->tm_year + 1900;
    if (year < 1970 || year > 2106 || tm->tm_mon < 0 || tm->tm_mon > 11 ||
        tm->tm_mday < 1 || tm->tm_mday > month_days(tm->tm_mon, year) ||
        tm->tm_hour < 0 || tm->tm_hour > 23 || tm->tm_min < 0 || tm->tm_min > 59 ||
        tm->tm_sec < 0 || tm->tm_sec > 59)
        return -1;
    time64_t days = rtc_year_days(tm->tm_mday, tm->tm_mon, year);
    for (int y = 1970; y < year; ++y)
        days += 365 + leap(y);
    return ((days * 24 + tm->tm_hour) * 60 + tm->tm_min) * 60 + tm->tm_sec;
}

void rtc_time64_to_tm(time64_t seconds, struct rtc_time *tm)
{
    *tm = (struct rtc_time){0};
    if (seconds < 0 || seconds > U32_MAX) {
        tm->tm_year = -1;
        return;
    }
    unsigned days = seconds / 86400;
    tm->tm_sec = seconds % 60;
    tm->tm_min = seconds / 60 % 60;
    tm->tm_hour = seconds / 3600 % 24;
    tm->tm_wday = (4 + days) % 7;
    int year = 1970;
    while (days >= (unsigned)(365 + leap(year))) {
        days -= 365 + leap(year);
        ++year;
    }
    tm->tm_year = year - 1900;
    tm->tm_yday = days;
    while (days >= (unsigned)month_days(tm->tm_mon, year))
        days -= month_days(tm->tm_mon++, year);
    tm->tm_mday = days + 1;
}
