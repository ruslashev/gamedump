// Translation of https://github.com/jaketodaro/discrete-interval-tree
// BORKEN

#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define min(a, b) ((a) < (b) ? (a) : (b))
#define max(a, b) ((a) > (b) ? (a) : (b))

#define i16 int16_t
#define N 1000
#define T INT16_MAX

struct node
{
    i16 start;
    i16 end;
    i16 left;
    i16 right;
    i16 parent;
};

i16 len = 0;
i16 root = T;
struct node nodes[N];

#define TEST_MAX_VAL 400
uint8_t bools[TEST_MAX_VAL + 1];

void printer(i16 x, int level)
{
    if (x == T)
        return;

    for (int i = 1; i <= level * 4; ++i)
        printf(" ");

    printf("[%d,%d]\n", nodes[x].start, nodes[x].end);

    printer(nodes[x].right, level + 1);
    printer(nodes[x].left, level + 1);
}

void print()
{
    printer(root, 0);
}

void blit(i16 start, i16 end)
{
    printf("blit [%d,%d]\n", start, end);
}

i16 new_node(i16 start, i16 end, i16 parent)
{
    i16 n = len++;
    printf("new_node(start=%d end=%d parent=%d) = %d\n", start, end, parent, n);
    nodes[n].start = start;
    nodes[n].end = end;
    nodes[n].left = T;
    nodes[n].right = T;
    nodes[n].parent = parent;
    return n;
}

bool point_contains(i16 x, i16 value)
{
    return value >= nodes[x].start && value <= nodes[x].end;
}

i16 absorb_left(i16 x, i16 y)
{
    nodes[x].start = nodes[y].start;
    nodes[x].left = nodes[y].left;

    if (nodes[x].left != T)
        nodes[nodes[x].left].parent = x;

    return x;
}

i16 absorb_right(i16 x, i16 y)
{
    printf("absorb_right(x=[%d,%d] y=[%d,%d])\n", nodes[x].start, nodes[x].end, nodes[y].start, nodes[y].end);

    nodes[x].end = nodes[y].end;
    nodes[x].right = nodes[y].right;

    if (nodes[x].right != T)
        nodes[nodes[x].right].parent = x;

    return x;
}

i16 add_value(i16 value)
{
    printf("add_value(value=%d)\n", value);

    i16 x = root;
    i16 value_node = T;

    while (value_node == T) {
        if (value < nodes[x].start - 1) {
            // value is somewhere to the left
            if (nodes[x].left != T) {
                x = nodes[x].left;
            } else {
                blit(value, value);
                value_node = nodes[x].left = new_node(value, value, x);
            }
        } else if (value == nodes[x].start - 1) {
            // value borders left
            if (nodes[x].left != T && value == nodes[nodes[x].left].end + 1) {
                // absorb left child
                blit(nodes[nodes[x].left].end + 1, nodes[x].start - 1);
                value_node = absorb_left(x, nodes[x].left);
            } else {
                blit(value, value);
                // just extend 1 to the left
                nodes[x].start = value;
                value_node = x;
            }
        } else if (point_contains(x, value)) {
            // value is contained in existing interval
            value_node = x;
        } else if (value == nodes[x].end + 1) {
            // value borders right
            if (nodes[x].right != T && value == nodes[nodes[x].right].start - 1) {
                // absorb right child
                blit(nodes[x].end + 1, nodes[nodes[x].right].start - 1);
                value_node = absorb_right(x, nodes[x].right);
            } else {
                blit(value, value);
                // just extend 1 to the right
                nodes[x].end = value;
                value_node = x;
            }
        } else if (value > nodes[x].end + 1){
            // value is somewhere to the right
            if (nodes[x].right != T) {
                x = nodes[x].right;
            } else {
                blit(value, value);
                value_node = nodes[x].right = new_node(value, value, x);
            }
        }
    }

    return value_node;
}

void insert(i16 start, i16 end)
{
    printf("\nInserting [%d,%d]\n", start, end);

    if (root == T) {
        root = new_node(start, end, T);
        blit(start, end);
        print();
        return;
    }

    i16 start_node = add_value(start);
    i16 end_node = add_value(end);

    absorb_right(start_node, end_node);
}

void gather_values(i16 x, i16* values, i16* len)
{
    values[(*len)++] = nodes[x].start;

    if (nodes[x].left != T)
        gather_values(nodes[x].left, values, len);

    if (nodes[x].right != T)
        gather_values(nodes[x].right, values, len);
}

void check_inequality(i16 x)
{
    i16 l = nodes[x].left;
    i16 r = nodes[x].right;

    if (l != T) {
        i16 *values = malloc(N * sizeof(i16));
        i16 len = 0;

        gather_values(l, values, &len);

        for (int i = 0; i < len; ++i)
            assert(values[i] <= nodes[x].start);

        free(values);

        check_inequality(l);
    }

    if (r != T) {
        i16 *values = malloc(N * sizeof(i16));
        i16 len = 0;

        gather_values(r, values, &len);

        for (int i = 0; i < len; ++i)
            assert(values[i] >= nodes[x].start);

        free(values);

        check_inequality(r);
    }
}

bool adjacent(i16 x, i16 y)
{
    i16 x0 = nodes[x].start;
    i16 x1 = nodes[x].end;
    i16 y0 = nodes[y].start;
    i16 y1 = nodes[y].end;

    printf("[%d,%d] ? [%d,%d]\n", x0, x1, y0, y1);

    return (x0 <= y1 + 1) && (y0 <= x1 + 1);
}

void check_isolation()
{
    i16 *values = malloc(N * sizeof(i16));
    i16 len = 0;

    gather_values(root, values, &len);

    for (int x = 0; x < len; ++x)
        for (int y = x + 1; y < len; ++y)
            assert(!adjacent(x, y));
}

void test()
{
    int num_tests = 0;

    while (1) {
        ++num_tests;
        printf("test=%d\n", num_tests);

        srand(num_tests);

        root = T;
        len = 0;
        memset(bools, 0, TEST_MAX_VAL + 1);

        while (1) {
            int start = 1 + rand() % 200;
            int end = start + rand() % 200;

            end = min(end, TEST_MAX_VAL);

            insert(start, end);

            printf("inserted [%d,%d]\n", start, end);

            print();

            check_inequality(root);
            check_isolation();

            if (nodes[root].start == 0 && nodes[root].end == TEST_MAX_VAL)
                break;
        }
    }
}

int main()
{
    /* insert(1, 1); */
    /* insert(3, 3); */
    /* insert(5, 5); */
    /* insert(6, 6); */
    /* insert(7, 7); */
    /* insert(9, 12); */
    /* insert(14, 16); */
    /* insert(13, 18); */
    /* insert(2, 2); */

    /* insert(1, 2); */
    /* insert(5, 6); */
    /* insert(3, 4); */

    insert(3, 4);
    insert(1, 6);
    print();

    /* test(); */
}
