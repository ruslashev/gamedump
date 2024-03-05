// Discrete Interval Encoding Tree based on an AVL tree
// Based on https://github.com/tcsprojects/camldiets

#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <err.h>
#include <stdlib.h>
#include <string.h>

#define i16 int16_t
#define max(a, b) ((a) > (b) ? (a) : (b))

#define TEST_MAX_VAL 30
#define START_RAND 20
#define SIZE_RAND 10
#define MASK_LEN (TEST_MAX_VAL + 1)
uint8_t mask[MASK_LEN];
uint8_t test_mask[MASK_LEN];

void blit(i16 start, i16 end);
void debug_insert(i16 start, i16 end);

struct node {
    i16 start;
    i16 end;
    i16 height;
    i16 left;
    i16 right;
};

const i16 bal_const = 1;

#define N 1000
#define T INT16_MAX

i16 len = 0;
i16 root = T;
struct node nodes[N];

i16 height(i16 tree)
{
    if (tree == T)
        return 0;

    return nodes[tree].height;
}

i16 height_join(i16 left, i16 right)
{
    return 1 + max(height(left), height(right));
}

i16 new_node(i16 start, i16 end, i16 height, i16 left, i16 right)
{
    i16 n = len;

    assert(n < N);

    printf("create_node(start=%d end=%d height=%d left=%d right=%d) = %d\n",
            start, end, height, left, right, n);

    len += 1;

    nodes[n].start = start;
    nodes[n].end = end;
    nodes[n].height = height;
    nodes[n].left = left;
    nodes[n].right = right;

    return n;
}

i16 create(i16 start, i16 end, i16 l, i16 r)
{
    return new_node(start, end, height_join(l, r), l, r);
}

i16 balance(i16 start, i16 end, i16 l, i16 r)
{
    i16 hl = height(l);
    i16 hr = height(r);

    if (hl > hr + bal_const) {
        if (l == T)
            err(0, "Node.balance");

        i16 ls = nodes[l].start;
        i16 le = nodes[l].end;
        i16 ll = nodes[l].left;
        i16 lr = nodes[l].right;

        if (height(ll) >= height(lr)) {
            return create(ls, le, ll, create(start, end, lr, r));
        } else {
            if (lr == T)
                err(0, "Node.balance");

            i16 lrs = nodes[lr].start;
            i16 lre = nodes[lr].end;
            i16 lrl = nodes[lr].left;
            i16 lrr = nodes[lr].right;

            return create(
                lrs,
                lre,
                create(ls, le, ll, lrl),
                create(start, end, lrr, r)
            );
        }
    } else if (hr > hl + bal_const) {
        if (r == T)
            err(0, "Node.balance");

        i16 rs = nodes[r].start;
        i16 re = nodes[r].end;
        i16 rl = nodes[r].left;
        i16 rr = nodes[r].right;

        if (height(rr) >= height(rl)) {
            return create(rs, re, create(start, end, l, rl), rr);
        } else {
            if (rl == T)
                err(0, "Node.balance");

            i16 rls = nodes[rl].start;
            i16 rle = nodes[rl].end;
            i16 rll = nodes[rl].left;
            i16 rlr = nodes[rl].right;

            return create(
                rls,
                rle,
                create(start, end, l, rll),
                create(rs, re, rlr, rr)
            );
        }
    } else {
        i16 h = (hl >= hr) ? hl + 1 : hr + 1;
        return new_node(start, end, h, l, r);
    }
}

i16 add(i16 tree, bool left, i16 start, i16 end)
{
    if (tree == T)
        return new_node(start, end, 1, T, T);

    i16 s = nodes[tree].start;
    i16 e = nodes[tree].end;
    i16 l = nodes[tree].left;
    i16 r = nodes[tree].right;

    if (left) {
        return balance(
            s,
            e,
            add(l, left, start, end),
            r
        );
    } else {
        return balance(
            s,
            e,
            l,
            add(r, left, start, end)
        );
    }
}

i16 join(i16 start, i16 end, i16 l, i16 r)
{
    if (l == T)
        return add(r, true, start, end);

    if (r == T)
        return add(l, false, start, end);

    i16 ls = nodes[l].start;
    i16 le = nodes[l].end;
    i16 lh = nodes[l].height;
    i16 ll = nodes[l].left;
    i16 lr = nodes[l].right;

    i16 rs = nodes[r].start;
    i16 re = nodes[r].end;
    i16 rh = nodes[r].height;
    i16 rl = nodes[r].left;
    i16 rr = nodes[r].right;

    if (lh > rh + bal_const)
        return balance(ls, le, ll, join(start, end, lr, r));
    else if (rh > lh + bal_const)
        return balance(rs, re, join(start, end, l, rl), rr);
    else
        return create(start, end, l, r);
}

void find_del_left(i16 tree, i16 start, i16 def_blit_end, i16* outs, i16* outl)
{
    if (tree == T) {
        *outs = start;
        *outl = T;
        blit(start, def_blit_end);
        return;
    }

    i16 s = nodes[tree].start;
    i16 e = nodes[tree].end;
    i16 l = nodes[tree].left;
    i16 r = nodes[tree].right;

    if (start > e + 1) {
        i16 news;
        i16 newr;
        find_del_left(r, start, def_blit_end, &news, &newr);

        *outs = news;
        *outl = join(s, e, l, newr);
    } else if (start < s) {
        find_del_left(l, start, def_blit_end, outs, outl);
    } else {
        blit(e + 1, def_blit_end);
        *outs = s;
        *outl = l;
    }
}

void find_del_right(i16 tree, i16 end, i16 def_blit_start, i16* oute, i16* outr)
{
    if (tree == T) {
        *oute = end;
        *outr = T;
        blit(def_blit_start, end);
        return;
    }

    i16 s = nodes[tree].start;
    i16 e = nodes[tree].end;
    i16 l = nodes[tree].left;
    i16 r = nodes[tree].right;

    if (end < s - 1) {
        i16 newe;
        i16 newl;
        find_del_right(l, end, def_blit_start, &newe, &newl);

        *oute = newe;
        *outr = join(s, e, newl, r);
    } else if (end > e) {
        find_del_right(r, end, def_blit_start, oute, outr);
    } else {
        blit(def_blit_start, s - 1);
        *oute = e;
        *outr = r;
    }
}

i16 insert_range(i16 tree, i16 start, i16 end)
{
    if (tree == T) {
        blit(start, end);
        return new_node(start, end, 1, T, T);
    }

    i16 s = nodes[tree].start;
    i16 e = nodes[tree].end;
    i16 l = nodes[tree].left;
    i16 r = nodes[tree].right;

    if (end < s - 1) {
        i16 new = insert_range(l, start, end);
        return join(s, e, new, r);
    } else if (start > e + 1) {
        i16 new = insert_range(r, start, end);
        return join(s, e, l, new);
    } else {
        i16 def_blit_start = e + 1;
        i16 def_blit_end = s - 1;

        i16 news, newl;
        if (start >= s) {
            news = s;
            newl = l;
        } else {
            find_del_left(l, start, def_blit_end, &news, &newl);
        };

        i16 newe, newr;
        if (end <= e) {
            newe = e;
            newr = r;
        } else {
            find_del_right(r, end, def_blit_start, &newe, &newr);
        };

        return join(news, newe, newl, newr);
    }
}

void insert(i16 start, i16 end)
{
    root = insert_range(root, start, end);

    debug_insert(start, end);
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

    printf("[%d,%d]\n", nodes[x].start, nodes[x].end);

    printer(nodes[x].right, level + 1, 1);
    printer(nodes[x].left, level + 1, -1);
}

void print()
{
    printer(root, 0, 0);
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
            assert(nodes[values[i]].start <= nodes[x].start);

        free(values);

        check_inequality(l);
    }

    if (r != T) {
        i16 *values = malloc(N * sizeof(i16));
        i16 len = 0;

        gather_indices(r, values, &len);

        for (int i = 0; i < len; ++i)
            assert(nodes[values[i]].start >= nodes[x].start);

        free(values);

        check_inequality(r);
    }
}

bool overlapping_or_adjacent(i16 x, i16 y)
{
    i16 x0 = nodes[x].start;
    i16 x1 = nodes[x].end;
    i16 y0 = nodes[y].start;
    i16 y1 = nodes[y].end;

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

    free(values);
}

i16 calc_height(i16 x)
{
    if (x == T)
        return 0;

    i16 l = calc_height(nodes[x].left);
    i16 r = calc_height(nodes[x].right);

    return 1 + max(l, r);
}

void check_height(i16 x)
{
    assert(calc_height(x) == nodes[x].height);

    if (nodes[x].left != T)
        check_height(nodes[x].left);

    if (nodes[x].right != T)
        check_height(nodes[x].right);
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

void print_masks()
{
    print_mask(mask);
    print_mask(test_mask);
    print_mask_indices();
}

void check_masks()
{
    for (i16 i = 0; i < MASK_LEN; ++i) {
        bool mask_equal = mask[i] == test_mask[i];

        if (!mask_equal)
            print_masks();

        assert(mask_equal);
    }
}

void run_checks()
{
    check_inequality(root);
    check_isolation();
    check_height(root);
    check_masks();
}

void clear()
{
    root = T;
    len = 0;
    memset(mask, 0, MASK_LEN);
    memset(test_mask, 0, MASK_LEN);

    static int test_case = 1;
    for (int i = 0; i < 80; ++i)
        printf("#");
    printf("\n# test case %d\n", test_case++);
}

void blit(i16 start, i16 end)
{
    for (i16 i = start; i <= end; ++i)
        mask[i] = 2;
}

void insert_test_mask(i16 start, i16 end)
{
    for (i16 i = start; i <= end; ++i)
        if (test_mask[i] == 0)
            test_mask[i] = 2;
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

void debug_insert(i16 start, i16 end)
{
    insert_test_mask(start, end);
    print();
    run_checks();
    freeze_masks();
    printf("\n");
}

int main()
{
    clear();
    insert(2, 5);
    insert(6, 8);

    clear();
    insert(3, 5);
    insert(1, 7);

    clear();
    insert(1, 3);
    insert(7, 9);
    insert(13, 15);
    insert(19, 21);
    insert(24, 26);
    insert(2, 25);

    clear();
    insert(2, 2);
    insert(4, 4);
    insert(6, 6);
    insert(8, 8);
    insert(3, 7);

    clear();
    insert(1, 1);
    insert(3, 3);
    insert(5, 5);
    insert(6, 6);
    insert(7, 7);
    insert(9, 12);
    insert(14, 16);
    insert(13, 18);
    insert(2, 2);
}
