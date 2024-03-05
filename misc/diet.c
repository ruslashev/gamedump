// Discrete Interval Encoding Tree (DIET)
// Based on https://github.com/typelevel/cats-collections
// File core/src/main/scala/cats/collections/Diet.scala

#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define min(a, b) ((a) < (b) ? (a) : (b))
#define max(a, b) ((a) > (b) ? (a) : (b))

#define i16 int16_t
#define N 10000
#define T INT16_MAX

#define TEST_MAX_VAL 26
#define START_RAND 18
#define SIZE_RAND 14
#define MASK_LEN (TEST_MAX_VAL + 1)
uint8_t mask[MASK_LEN];
uint8_t test_mask[MASK_LEN];

struct node
{
    i16 low;
    i16 high;
    i16 left;
    i16 right;
};

i16 len = 0;
i16 root = T;
struct node nodes[N];

void blit(i16 start, i16 end)
{
    for (i16 i = start; i <= end; ++i)
        mask[i] = 2;
}

void insert_test_mask(i16 low, i16 high)
{
    for (i16 i = low; i <= high; ++i)
        if (test_mask[i] == 0)
            test_mask[i] = 2;
}

void print_mask(uint8_t* mask)
{
    for (int i = 0; i < MASK_LEN; ++i)
        printf("%d", mask[i]);
    printf("\n");
}

void print_mask_indices()
{
    for (int i = 0; i < MASK_LEN; ++i)
        printf("%d", i % 10);
    printf("\n");

    for (int i = 0; i < MASK_LEN; ++i) {
        int d = i / 10;

        if (d == 0)
            printf(" ");
        else
            printf("%d", d);
    }
    printf("\n");
}

void check_masks()
{
    print_mask(mask);
    print_mask(test_mask);
    print_mask_indices();

    for (i16 i = 0; i < MASK_LEN; ++i)
        assert(mask[i] == test_mask[i]);
}

void freeze_masks()
{
    for (i16 i = 0; i < MASK_LEN; ++i) {
        if (mask[i] == 2)
            mask[i] = 1;

        if (test_mask[i] == 2)
            test_mask[i] = 1;
    }
}

void printer(i16 x, int level, int dir)
{
    if (x == T)
        return;

    for (int i = 1; i <= level * 4 - 1; ++i)
        printf(" ");

    if (dir == -1)
        printf("l");
    else if (dir == 1)
        printf("r");

    printf("[%d,%d]\n", nodes[x].low, nodes[x].high);

    printer(nodes[x].right, level + 1, 1);
    printer(nodes[x].left, level + 1, -1);
}

void print()
{
    printer(root, 0, 0);
}

i16 new_node(i16 low, i16 high, i16 left, i16 right)
{
    i16 n = len++;
    nodes[n].low = low;
    nodes[n].high = high;
    nodes[n].left = left;
    nodes[n].right = right;
    return n;
}

i16 less_than_or_equal(i16 x, i16 low, i16 blit_low, i16 blit_high, i16* out_low)
{
    if (x == T) {
        *out_low = low;
        return x;
    }

    if (low > nodes[x].high + 1) {
        i16 new_blit_low = nodes[x].high + 1;
        i16 new_blit_high = blit_low;

        i16 new_low;
        i16 r2 = less_than_or_equal(nodes[x].right, low, new_blit_low, new_blit_high, &new_low);

        *out_low = min(low, new_low);

        return new_node(nodes[x].low, nodes[x].high, nodes[x].left, r2);
    }
    else if (low >= nodes[x].low) {
        // TODO: change blit_high to low or something
        blit(nodes[x].high + 1, blit_high);

        *out_low = nodes[x].low;
        return nodes[x].left;
    }
    else {
        blit(nodes[x].high + 1, blit_high);
        i16 new_blit_high = nodes[x].low - 1;

        if (nodes[x].left == T)
            blit(blit_low, new_blit_high);

        return less_than_or_equal(nodes[x].left, low, blit_low, new_blit_high, out_low);
    }
}

i16 greater_than_or_equal(i16 x, i16 high, i16 blit_low, i16 blit_high, i16* out_high)
{
    if (x == T) {
        *out_high = high;
        return x;
    }

    if (high < nodes[x].low - 1) {
        i16 new_blit_low = blit_high;
        i16 new_blit_high = nodes[x].low - 1;

        i16 new_high;
        i16 l2 = greater_than_or_equal(nodes[x].left, high, new_blit_low, new_blit_high, &new_high);

        *out_high = max(high, new_high);

        return new_node(nodes[x].low, nodes[x].high, l2, nodes[x].right);
    }
    else if (high <= nodes[x].high) {
        /* blit(nodes[x].high - 1, nodes[x].low - 1); */
        blit(blit_low, nodes[x].low - 1);
        /* blit(high, nodes[x].low - 1); */

        *out_high = nodes[x].high;
        return nodes[x].right;
    }
    else {
        blit(blit_low, nodes[x].low - 1);
        i16 new_blit_low = nodes[x].high + 1;

        if (nodes[x].right == T)
            blit(new_blit_low, blit_high);

        return greater_than_or_equal(nodes[x].right, high, new_blit_low, blit_high, out_high);
    }
}

i16 insert_range(i16 x, i16 low, i16 high)
{
    if (x == T) {
        blit(low, high);
        return new_node(low, high, T, T);
    }

    i16 ll, lh;
    i16 rl, rh;
    i16 default_blit_low;
    i16 default_blit_high;

    if (nodes[x].low < low) {
        ll = nodes[x].low;
        lh = nodes[x].high;
        rl = low;
        rh = high;
        /* default_blit_low = lh + 1; */
        /* default_blit_high = rh; */
        default_blit_low = max(rl, lh + 1);
        default_blit_high = rh;
    } else {
        ll = low;
        lh = high;
        rl = nodes[x].low;
        rh = nodes[x].high;
        default_blit_low = ll;
        default_blit_high = min(lh, rl - 1);
        /* default_blit_low = ll; */
        /* default_blit_high = nodes[x].low - 1; */
    }

    i16 r1l;
    i16 r1h;
    i16 r2l;
    i16 r2h;

    if (lh >= rl || lh + 1 == rl) {
        if (low >= nodes[x].low && high <= nodes[x].high)
            return x;

        r1l = ll;
        r1h = max(lh, rh);

        i16 new_low;
        i16 new_left = less_than_or_equal(nodes[x].left, r1l, default_blit_low, default_blit_high,
                &new_low);

        i16 new_high;
        i16 new_right = greater_than_or_equal(nodes[x].right, r1h, default_blit_low,
                default_blit_high, &new_high);

        if (nodes[x].left == T && nodes[x].right == T)
            blit(default_blit_low, default_blit_high);

        nodes[x].low = new_low;
        nodes[x].high = new_high;
        nodes[x].left = new_left;
        nodes[x].right = new_right;

        return x;
    } else {
        r1l = ll;
        r1h = lh;
        r2l = rl;
        r2h = rh;
        if (r1l == nodes[x].low && r1h == nodes[x].high) {
            i16 right = insert_range(nodes[x].right, r2l, r2h);
            nodes[x].right = right;
            return x;
        } else {
            i16 left = insert_range(nodes[x].left, r1l, r1h);
            nodes[x].left = left;
            return x;
        }
    }
}

void insert(i16 low, i16 high)
{
    root = insert_range(root, low, high);

    insert_test_mask(low, high);
    check_masks();
    freeze_masks();

    print();
    printf("\n");
}

void clear()
{
    root = T;
    len = 0;
    memset(mask, 0, MASK_LEN);
    memset(test_mask, 0, MASK_LEN);
}

void gather_indices(i16 x, i16* values, i16* len)
{
    values[(*len)++] = x;

    if (nodes[x].left != T)
        gather_indices(nodes[x].left, values, len);

    if (nodes[x].right != T)
        gather_indices(nodes[x].right, values, len);
}

void check_inequality(i16 x)
{
    i16 l = nodes[x].left;
    i16 r = nodes[x].right;

    if (l != T) {
        i16 *values = malloc(N * sizeof(i16));
        i16 len = 0;

        gather_indices(l, values, &len);

        for (int i = 0; i < len; ++i)
            assert(nodes[values[i]].low <= nodes[x].low);

        free(values);

        check_inequality(l);
    }

    if (r != T) {
        i16 *values = malloc(N * sizeof(i16));
        i16 len = 0;

        gather_indices(r, values, &len);

        for (int i = 0; i < len; ++i)
            assert(nodes[values[i]].low >= nodes[x].low);

        free(values);

        check_inequality(r);
    }
}

bool overlapping_or_adjacent(i16 x, i16 y)
{
    i16 x0 = nodes[x].low;
    i16 x1 = nodes[x].high;
    i16 y0 = nodes[y].low;
    i16 y1 = nodes[y].high;

    return (x0 <= y1 + 1) && (y0 <= x1 + 1);
}

void check_isolation()
{
    i16 *values = malloc(N * sizeof(i16));
    i16 len = 0;

    gather_indices(root, values, &len);

    for (int x = 0; x < len; ++x)
        for (int y = x + 1; y < len; ++y)
            assert(!overlapping_or_adjacent(values[x], values[y]));
}

void test()
{
    int test_num = 0;

    while (1) {
        ++test_num;
        printf("test=%d\n", test_num);

        srand(test_num);

        clear();

        while (1) {
            int low = 1 + rand() % START_RAND;
            int high = low + rand() % SIZE_RAND;

            high = min(high, TEST_MAX_VAL);

            insert(low, high);

            check_inequality(root);
            check_isolation();

            bool filled = nodes[root].low == 1 && nodes[root].high == TEST_MAX_VAL;
            bool overflow = len == N - 1;
            if (filled || overflow)
                break;
        }
    }
}

void header()
{
    static int test_case_num = 1;

    for (int i = 0; i < 80; ++i)
        printf("#");

    printf("\n# Test case %d\n", test_case_num++);
}

int main()
{
    header();
    insert(1, 1);
    insert(3, 3);
    insert(5, 5);
    insert(6, 6);
    insert(7, 7);
    insert(9, 12);
    insert(14, 16);
    insert(13, 18);
    insert(2, 2);
    clear();

    header();
    insert(2, 2);
    insert(4, 4);
    insert(6, 6);
    insert(8, 8);
    insert(3, 7);
    clear();

    insert(2, 3);
    insert(6, 7);
    insert(10, 11);
    insert(4, 9);
    clear();

    header();
    insert(8, 8);
    insert(6, 6);
    insert(4, 4);
    insert(2, 2);
    insert(3, 7);
    clear();

    header();
    insert(2, 5);
    insert(6, 9);
    clear();

    header();
    insert(6, 9);
    insert(2, 5);
    clear();

    header();
    insert(1, 5);
    insert(9, 13);
    insert(3, 11);
    clear();

    header();
    insert(10, 11);
    insert(9, 12);

    header();
    insert(24,26);
    insert(10,11);
    insert( 4, 5);
    insert(17,18);
    insert( 1, 2);
    insert( 7, 8);
    insert(13,15);
    insert(20,22);
    puts("INSERT");
    insert(9,12);
    clear();

    header();
    insert(10,15);
    insert(17,26);
    insert(15,18);
    clear();

    header();
    insert( 2,15);
    insert(16,19);
    clear();

    header();
    insert(16,19);
    insert( 2,15);
    clear();

    header();
    insert( 2,26);
    insert( 1,13);
    clear();

    test();
}
