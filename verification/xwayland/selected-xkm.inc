/* Included by the Xwayland 24.1.6 server, with its native XKB types. */
#include "jnox-selected-xkm-data.h"
#include <string.h>
#include "jnox-selected-xkm.h"

/* Single-threaded X server. Captured by the native guest observer. */
volatile unsigned long jnox_xkm_witness[10];

size_t
jnox_selected_xkb_size(void)
{
    return sizeof(jnox_xkb_wire);
}

Bool
jnox_selected_xkb_wire(const void *data, size_t size)
{
    return data && size == sizeof(jnox_xkb_wire) &&
           memcmp(data, jnox_xkb_wire, size) == 0;
}

/* Return only the server's immutable canonical bytes. Another process can
 * retain a writable alias to the received backing even for a read-only FD. */
const char *
jnox_selected_xkb_identify(const void *data, size_t size)
{
    return jnox_selected_xkb_wire(data, size) ? (const char *)jnox_xkb_wire : NULL;
}

static Bool
jnox_expected_key(XkbDescPtr map, KeyCode key, KeySym sym)
{
    if (!map->map->key_sym_map || !map->map->types || !map->map->syms ||
        key < map->min_key_code || key > map->max_key_code ||
        XkbKeyNumGroups(map, key) != 1 || XkbKeyNumSyms(map, key) != 1 ||
        XkbKeySymEntry(map, key, 0, 0) != sym ||
        !map->map->modmap || map->map->modmap[key] != 0)
        return FALSE;
    XkbKeyTypePtr type = XkbKeyKeyType(map, key, 0);
    return type && type->num_levels == 1 && !type->mods.mask &&
           !type->mods.real_mods && !type->mods.vmods;
}

static XkbDescPtr
jnox_read_selected_xkm(void)
{
    XkbDescPtr map = NULL;
    FILE *file = fmemopen((void *)jnox_xkm_bytes, sizeof(jnox_xkm_bytes), "rb");
    if (!file)
        return NULL;
    jnox_xkm_witness[8]++;
    const unsigned need = XkmKeyNamesMask | XkmTypesMask | XkmSymbolsMask;
    unsigned missing = XkmReadFile(file, need, XkmAllIndicesMask, &map);
    fclose(file);
    if ((missing & need) || !map || !map->map || !map->ctrls ||
        !jnox_expected_key(map, 45, XK_k) || !jnox_expected_key(map, 53, XK_x)) {
        if (map)
            XkbFreeKeyboard(map, XkbAllComponentsMask, TRUE);
        LogMessage(X_ERROR, "JNOX: selected XKM failed its keyboard contract\n");
        return NULL;
    }
    jnox_xkm_witness[2] = map->defined;
    jnox_xkm_witness[3] = XkbKeySymEntry(map, 45, 0, 0);
    jnox_xkm_witness[4] = XkbKeySymEntry(map, 53, 0, 0);
    jnox_xkm_witness[5] = map->map->modmap[45];
    jnox_xkm_witness[6] = map->map->modmap[53];
    return map;
}

XkbDescPtr
XkbCompileKeymapFromString(DeviceIntPtr dev, const char *keymap, int length)
{
    if (!dev || !keymap || length != (int)sizeof(jnox_xkb_wire) - 1 ||
        memcmp(keymap, jnox_xkb_wire, (size_t)length)) {
        LogMessage(X_ERROR, "JNOX: unsupported XKB keymap string\n");
        jnox_xkm_witness[7]++;
        return NULL;
    }
    XkbDescPtr map = jnox_read_selected_xkm();
    if (map)
        jnox_xkm_witness[1]++;
    return map;
}

XkbDescPtr
XkbCompileKeymap(DeviceIntPtr dev, XkbRMLVOSet *r)
{
    if (!dev || !r || !r->rules || strcmp(r->rules, "jnox") ||
        !r->model || strcmp(r->model, "jnox") ||
        !r->layout || strcmp(r->layout, "jnox") ||
        (r->variant && *r->variant) || (r->options && *r->options)) {
        LogMessage(X_ERROR, "JNOX: unsupported XKB rules/model/layout/variant/options\n");
        jnox_xkm_witness[9]++;
        return NULL;
    }
    XkbDescPtr map = jnox_read_selected_xkm();
    if (map)
        jnox_xkm_witness[0]++;
    return map;
}

unsigned
XkbDDXLoadKeymapByNames(DeviceIntPtr dev, XkbComponentNamesPtr names,
                        unsigned want, unsigned need, XkbDescPtr *map,
                        char *name, int name_len)
{
    *map = NULL;
    if (name && name_len > 0)
        name[0] = '\0';
    LogMessage(X_ERROR, "JNOX: XKB component-name compilation unavailable\n");
    return 0;
}

Bool
XkbDDXNamesFromRules(DeviceIntPtr dev, const char *rules, XkbRF_VarDefsPtr defs,
                     XkbComponentNamesPtr names)
{
    return FALSE;
}
