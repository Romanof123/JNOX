// SPDX-License-Identifier: GPL-2.0-or-later
/* The original allocator owns cache, DMA and buffer references separately.
 * Its page-sized service-local allocations never address the kernel linear map. */
#include <jnox_rx_page_api.h>

enum { RX_PAGES = 33 };
static_assert(PAGE_SIZE == 4096);
static unsigned char storage[RX_PAGES][PAGE_SIZE] __aligned(PAGE_SIZE);
static struct page metadata[RX_PAGES];

static int live_slot(const struct page *page)
{
    unsigned int i;
    for (i = 0; i < RX_PAGES; ++i)
        if (page == &metadata[i] && atomic_read(&metadata[i]._refcount) > 0)
            return i;
    return -1;
}

unsigned long jnox_rx_span_v1(const void *p, unsigned int len)
{
    unsigned long base = (unsigned long)storage, addr = (unsigned long)p, offset;
    if (addr < base || addr - base >= sizeof(storage) || !len)
        return 0;
    offset = addr - base;
    if (len > PAGE_SIZE - offset % PAGE_SIZE ||
        atomic_read(&metadata[offset / PAGE_SIZE]._refcount) <= 0)
        return 0;
    return PAGE_SIZE + offset;
}

void *jnox_rx_resolve_v1(unsigned long handle, unsigned int len)
{
    unsigned long offset = handle - PAGE_SIZE;
    unsigned char *addr;
    if (offset >= sizeof(storage))
        return NULL;
    addr = &storage[offset / PAGE_SIZE][offset % PAGE_SIZE];
    return jnox_rx_span_v1(addr, len) == handle ? addr : NULL;
}

struct page *jnox_rx_head_page_v1(const void *p)
{
    unsigned long handle = jnox_rx_span_v1(p, 1);
    if (!handle)
        __builtin_trap();
    return &metadata[handle / PAGE_SIZE - 1];
}

void *jnox_rx_page_address_v1(const struct page *page)
{
    int i = live_slot(page);
    if (i < 0)
        __builtin_trap();
    return storage[i];
}

void jnox_rx_get_page_v1(struct page *page)
{
    if (live_slot(page) < 0 || atomic_read(&page->_refcount) == INT_MAX)
        __builtin_trap();
    atomic_inc(&page->_refcount);
}

void jnox_rx_put_page_v1(struct page *page)
{
    if (live_slot(page) < 0)
        __builtin_trap();
    atomic_dec(&page->_refcount);
}

bool jnox_rx_page_refill_v1(unsigned int size, struct page_frag *frag, gfp_t gfp)
{
    unsigned int i;
    (void)gfp;
    /* An empty page needs room for the original 16-byte virtnet_rq_dma.
     * Service storage is preallocated, including in GFP_ATOMIC context. */
    if (!frag || !size || size > PAGE_SIZE - 16)
        return false;
    if (frag->page) {
        if (live_slot(frag->page) < 0 || frag->size != PAGE_SIZE ||
            frag->offset > frag->size)
            __builtin_trap();
        if (size <= frag->size - frag->offset)
            return true;
        jnox_rx_put_page_v1(frag->page);
        frag->page = NULL;
        frag->size = frag->offset = 0;
    }
    for (i = 0; i < RX_PAGES; ++i) {
        if (atomic_read(&metadata[i]._refcount))
            continue;
        memset(storage[i], 0, PAGE_SIZE);
        atomic_set(&metadata[i]._refcount, 1);
        frag->page = &metadata[i];
        frag->offset = 0;
        frag->size = PAGE_SIZE;
        return true;
    }
    return false;
}
