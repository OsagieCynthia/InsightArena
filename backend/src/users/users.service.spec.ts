import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { NotFoundException } from '@nestjs/common';
import { Repository } from 'typeorm';
import { UsersService } from './users.service';
import { User } from './entities/user.entity';
import { Prediction } from '../predictions/entities/prediction.entity';
import { ListUserPredictionsDto } from './dto/list-user-predictions.dto';

describe('UsersService', () => {
  let service: UsersService;
  let repository: Repository<User>;
  let predictionsRepository: Repository<Prediction>;

  const mockUser: User = {
    id: '123e4567-e89b-12d3-a456-426614174000',
    stellar_address: 'GBRPYHIL2CI3WHZDTOOQFC6EB4RRJC3XNRBF7XNZFXNRBF7XNRBF7XN',
    username: 'testuser',
    avatar_url: null,
    total_predictions: 10,
    correct_predictions: 7,
    total_staked_stroops: '1000000',
    total_winnings_stroops: '500000',
    reputation_score: 85,
    season_points: 100,
    role: 'user',
    created_at: new Date('2024-01-01'),
    updated_at: new Date('2024-01-01'),
  };

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        UsersService,
        {
          provide: getRepositoryToken(User),
          useValue: {
            findOneBy: jest.fn(),
          },
        },
        {
          provide: getRepositoryToken(Prediction),
          useValue: {
            createQueryBuilder: jest.fn(),
          },
        },
      ],
    }).compile();

    service = module.get<UsersService>(UsersService);
    repository = module.get<Repository<User>>(getRepositoryToken(User));
    predictionsRepository = module.get<Repository<Prediction>>(
      getRepositoryToken(Prediction),
    );
  });

  it('should be defined', () => {
    expect(service).toBeDefined();
  });

  describe('findByAddress', () => {
    it('should return a user when found', async () => {
      const findOneByMock = jest
        .spyOn(repository, 'findOneBy')
        .mockResolvedValue(mockUser);

      const result = await service.findByAddress(mockUser.stellar_address);

      expect(result).toEqual(mockUser);
      expect(findOneByMock).toHaveBeenCalledWith({
        stellar_address: mockUser.stellar_address,
      });
    });

    it('should throw NotFoundException when user not found', async () => {
      jest.spyOn(repository, 'findOneBy').mockResolvedValue(null);

      await expect(
        service.findByAddress('NONEXISTENT_ADDRESS'),
      ).rejects.toThrow(NotFoundException);
    });

    it('should throw NotFoundException with descriptive message', async () => {
      jest.spyOn(repository, 'findOneBy').mockResolvedValue(null);
      const address = 'GBRPYHIL2CI3WHZDTOOQFC6EB4RRJC3XNRBF7XNZFXNRBF7XNRBF7XN';

      await expect(service.findByAddress(address)).rejects.toThrow(
        new NotFoundException(`User with address ${address} not found`),
      );
    });
  });

  describe('findPublicPredictionsByAddress', () => {
    it('should return only resolved-market predictions with outcome mapping', async () => {
      jest.spyOn(repository, 'findOneBy').mockResolvedValue(mockUser);

      const now = new Date('2025-02-01T00:00:00.000Z');
      const queryBuilder = {
        leftJoinAndSelect: jest.fn().mockReturnThis(),
        where: jest.fn().mockReturnThis(),
        andWhere: jest.fn().mockReturnThis(),
        orderBy: jest.fn().mockReturnThis(),
        skip: jest.fn().mockReturnThis(),
        take: jest.fn().mockReturnThis(),
        getManyAndCount: jest.fn().mockResolvedValue([
          [
            {
              id: 'pred-1',
              chosen_outcome: 'YES',
              stake_amount_stroops: '100',
              payout_claimed: false,
              payout_amount_stroops: '0',
              tx_hash: null,
              submitted_at: now,
              market: {
                id: 'mkt-1',
                title: 'Resolved YES market',
                end_time: now,
                resolved_outcome: 'YES',
                is_resolved: true,
                is_cancelled: false,
              },
            },
            {
              id: 'pred-2',
              chosen_outcome: 'NO',
              stake_amount_stroops: '200',
              payout_claimed: false,
              payout_amount_stroops: '0',
              tx_hash: null,
              submitted_at: now,
              market: {
                id: 'mkt-2',
                title: 'Resolved YES market',
                end_time: now,
                resolved_outcome: 'YES',
                is_resolved: true,
                is_cancelled: false,
              },
            },
          ],
          2,
        ]),
      };

      jest
        .spyOn(predictionsRepository, 'createQueryBuilder')
        .mockReturnValue(
          queryBuilder as unknown as ReturnType<
            Repository<Prediction>['createQueryBuilder']
          >,
        );

      const result = await service.findPublicPredictionsByAddress(
        mockUser.stellar_address,
        new ListUserPredictionsDto(),
      );

      expect(queryBuilder.andWhere).toHaveBeenCalledWith(
        'market.is_resolved = true',
      );
      expect(result.total).toBe(2);
      expect(result.data).toHaveLength(2);
      expect(result.data[0].outcome).toBe('correct');
      expect(result.data[1].outcome).toBe('incorrect');
    });

    it('should filter public predictions by outcome', async () => {
      jest.spyOn(repository, 'findOneBy').mockResolvedValue(mockUser);

      const now = new Date('2025-02-01T00:00:00.000Z');
      const queryBuilder = {
        leftJoinAndSelect: jest.fn().mockReturnThis(),
        where: jest.fn().mockReturnThis(),
        andWhere: jest.fn().mockReturnThis(),
        orderBy: jest.fn().mockReturnThis(),
        skip: jest.fn().mockReturnThis(),
        take: jest.fn().mockReturnThis(),
        getManyAndCount: jest.fn().mockResolvedValue([
          [
            {
              id: 'pred-1',
              chosen_outcome: 'YES',
              stake_amount_stroops: '100',
              payout_claimed: false,
              payout_amount_stroops: '0',
              tx_hash: null,
              submitted_at: now,
              market: {
                id: 'mkt-1',
                title: 'Resolved YES market',
                end_time: now,
                resolved_outcome: 'YES',
                is_resolved: true,
                is_cancelled: false,
              },
            },
            {
              id: 'pred-2',
              chosen_outcome: 'NO',
              stake_amount_stroops: '200',
              payout_claimed: false,
              payout_amount_stroops: '0',
              tx_hash: null,
              submitted_at: now,
              market: {
                id: 'mkt-2',
                title: 'Resolved YES market',
                end_time: now,
                resolved_outcome: 'YES',
                is_resolved: true,
                is_cancelled: false,
              },
            },
          ],
          2,
        ]),
      };

      jest
        .spyOn(predictionsRepository, 'createQueryBuilder')
        .mockReturnValue(
          queryBuilder as unknown as ReturnType<
            Repository<Prediction>['createQueryBuilder']
          >,
        );

      const result = await service.findPublicPredictionsByAddress(
        mockUser.stellar_address,
        { outcome: 'correct' } as ListUserPredictionsDto,
      );

      expect(result.total).toBe(2);
      expect(result.data).toHaveLength(1);
      expect(result.data[0].outcome).toBe('correct');
    });
  });
});
